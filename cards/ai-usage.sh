#!/bin/bash
# ai-usage.sh — AI service usage rows for insomnia dashboard cards.
#
#   ai-usage.sh all            every service below, each under its own header
#   ai-usage.sh codex          rows: one per Codex/ChatGPT account
#   ai-usage.sh claude         rows: one per Claude account
#   ai-usage.sh copilot        rows: one per gh-logged-in GitHub account
#   ai-usage.sh capture codex <name>    snapshot current ~/.codex login into the store
#   ai-usage.sh capture claude <name>   snapshot current Claude Code login into the store
#
# Setup: `make setup` copies this script to ~/.config/insomnia/ and adds a
# card that runs `ai-usage.sh all`. Needs jq and curl; Copilot rows also need
# the GitHub CLI (gh). Nothing here is sent anywhere but the providers' own
# usage endpoints, and every credential stays on this Mac. macOS only: it
# leans on BSD `date -r`, `stat -f`, `base64 -D`, and the `security` Keychain.
#
# Accounts: whatever you're logged into right now shows up automatically
# (Claude Code / Claude Desktop, the Codex CLI / Codex Desktop, every
# `gh auth login` account). To keep a second Claude or Codex account visible,
# log into it, run `ai-usage.sh capture <claude|codex> <name>`, then log back
# into your usual one. To list an account even while it isn't logged in,
# add `<service> <name>` to ~/.config/insomnia/ai-usage/accounts.conf; to turn
# an account off without deleting anything, add `<service> <name> hidden`.
#
# These are the same undocumented endpoints the providers' own apps use for
# their usage screens, so they can change without notice.
#
# Row protocol: account summaries use ok|name|detail / warn|... / bad|...;
# each usage window follows as bar|accent|label|percent|reset. Bars go amber
# at 75% used and red at 90%. Copilot accounts come from `gh auth status`;
# codex/claude accounts are live app/CLI logins plus captured snapshots.
#
# Claude Desktop is read from its active OAuth session (running child first,
# encrypted app cache next, legacy browser cookies last). Codex Desktop and
# the Codex CLI share ~/.codex/auth.json; live accounts use that shared login.
# Captured snapshots remain available for additional accounts. Refresh tokens
# rotate, so a snapshot whose account is logged out everywhere eventually
# decays and is tagged `stale <age>` rather than passing as a live reading.
#
# Refresh tokens are single-use and shared with the CLIs that own them, so a
# token is refreshed only once its access token has expired or been refused
# with a 401 — never because of a timeout, 429, or 5xx — and always under a
# lock, since the TUI and the menu bar app both run this every 30s.

PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
umask 077

for bin in jq curl; do
  command -v "$bin" >/dev/null 2>&1 \
    || { echo "warn|ai usage|needs $bin (brew install $bin)"; exit 0; }
done

DIR="${XDG_CONFIG_HOME:-$HOME/.config}/insomnia/ai-usage"
CACHE="$DIR/cache"
TREND="$DIR/trend"
mkdir -p "$DIR/codex" "$DIR/claude" "$CACHE" "$TREND"
chmod 700 "$DIR"

CODEX_CLIENT_ID="app_EMoamEEZ73f0CkXaXp7hrann"          # public, from openai/codex source
CLAUDE_CLIENT_ID="9d1c250a-e61b-44d9-88ed-5944d1962f5e" # public, Claude Code's OAuth client

CURL="curl -sf -m 15"
LIVE_TTL=${AI_USAGE_TTL:-30}
SNAPSHOT_TTL=${AI_USAGE_SNAPSHOT_TTL:-240}
LIVE_RETRY=${AI_USAGE_RETRY:-60}
SNAPSHOT_RETRY=${AI_USAGE_SNAPSHOT_RETRY:-900}
LOCK_WAIT=20   # seconds to wait for another run's refresh before giving up
LOCK_STALE=120 # a lock this old outlived any refresh (a few 15s calls)

# http <curl args...> — curl's stdin carries the secret part (a `-K -`
# config with the auth header, or a `--data-binary @-` body), so tokens stay
# off argv where any `ps` could read them. Prints the body on 2xx; returns 2
# on 401 so callers can tell a refused token from a flaky network, else 1.
http() {
  local out code
  out=$(curl -s -m 15 -w '\n%{http_code}' "$@") || return 1
  code=${out##*$'\n'}
  case $code in
    2??) printf '%s' "${out%$'\n'*}" ;;
    401) return 2 ;;
    *)   return 1 ;;
  esac
}

# headers <header...> — a curl config on stdout for `http -K -`.
headers() {
  local h
  for h in "$@"; do printf 'header = "%s"\n' "$h"; done
}

# with_lock <name> <cmd...> — run cmd while holding $CACHE/<name>.lock. Two
# runs refreshing the same single-use refresh token at once can get the
# whole token family revoked. A mkdir lock records the owning script's pid,
# so one left by a killed run is taken over (or after LOCK_STALE regardless).
with_lock() (
  lock="$CACHE/$1.lock"; shift
  waited=0
  until mkdir "$lock" 2>/dev/null; do
    owner=$(cat "$lock/pid" 2>/dev/null)
    age=$(( $(date +%s) - $(stat -f %m "$lock" 2>/dev/null || date +%s) ))
    if { [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null; } || [ "$age" -ge "$LOCK_STALE" ]; then
      # rename first so two waiters can't both clear it and both get in
      mv "$lock" "$lock.dead.$$" 2>/dev/null && rm -rf "$lock.dead.$$"
      continue
    fi
    [ "$waited" -ge "$LOCK_WAIT" ] && exit 1
    sleep 1; waited=$(( waited + 1 ))
  done
  trap 'rm -rf "$lock"' EXIT
  printf '%s' "$$" > "$lock/pid"
  "$@"
)

dot() { # dot <pct-used>
  local p=${1%%.*}
  if [ "${p:-0}" -ge 90 ] 2>/dev/null; then echo bad
  elif [ "${p:-0}" -ge 75 ] 2>/dev/null; then echo warn
  else echo ok; fi
}

when_full() { # when_full <epoch> -> "Sun Sep 6 21:51 CDT"
  local year
  year=$(date -r "$1" '+%Y' 2>/dev/null) || return
  if [ "$year" = "$(date '+%Y')" ]; then
    date -r "$1" '+%a %b %-d %H:%M %Z' 2>/dev/null
  else
    date -r "$1" '+%a %b %-d %Y %H:%M %Z' 2>/dev/null
  fi
}

cache_fresh() { # cache_fresh <key> <ttl-seconds>
  local f age
  f="$CACHE/$1.json"
  [ -f "$f" ] || return 1
  age=$(( $(date +%s) - $(stat -f %m "$f") ))
  [ "$age" -lt "$2" ] && cat "$f"
}

cache_put() { # stdin -> cache
  local tmp
  tmp="$CACHE/$1.json.tmp.$$"
  cat > "$tmp" && mv "$tmp" "$CACHE/$1.json" \
    || { rm -f "$tmp"; return 1; }
}

cache_stale() { cat "$CACHE/$1.json" 2>/dev/null; }

retry_waiting() { # retry_waiting <key> <seconds>
  local f age
  f="$CACHE/$1.failed"
  [ -f "$f" ] || return 1
  age=$(( $(date +%s) - $(stat -f %m "$f") ))
  [ "$age" -lt "$2" ]
}

fetch() { # fetch <key> <ttl> <retry-seconds> <fn> <args...>
  local key ttl retry out
  key=$1; ttl=$2; retry=$3; shift 3
  if out=$(cache_fresh "$key" "$ttl"); then printf '%s\n' "$out"; return 0; fi
  if retry_waiting "$key" "$retry"; then cache_stale "$key"; return; fi
  if out=$("$@") && [ -n "$out" ] && printf '%s' "$out" | jq -e . >/dev/null 2>&1; then
    printf '%s' "$out" | cache_put "$key"
    rm -f "$CACHE/$key.failed"
    printf '%s\n' "$out"; return 0
  fi
  touch "$CACHE/$key.failed"
  cache_stale "$key"
}

# staleness <key> — age tag for a row whose fetches have stopped landing.
# Empty while refreshes succeed (cache is rewritten well inside STALE_AFTER);
# once credentials die, fetch keeps serving the last good response, and this
# is what stops that snapshot from passing as a live reading.
STALE_AFTER=900
staleness() {
  local f age
  f="$CACHE/$1.json"
  [ -f "$f" ] || return 0
  age=$(( $(date +%s) - $(stat -f %m "$f") ))
  [ "$age" -lt "$STALE_AFTER" ] && return 0
  if   [ "$age" -lt 3600 ];  then printf 'stale %dm' $(( age / 60 ))
  elif [ "$age" -lt 86400 ]; then printf 'stale %dh' $(( age / 3600 ))
  else printf 'stale %dd' $(( age / 86400 )); fi
}

# until_str <epoch> — a short countdown ("6h", "45m", "2d") to go with the
# absolute reset clock time; blank once the epoch is past or missing, since
# "resets Mon 09:00" alone means something different at Sun 11pm than at
# Mon 08:55 and the countdown removes that mental math.
until_str() {
  local now d
  now=$(date +%s)
  d=$(( ${1:-0} - now ))
  [ "$d" -gt 0 ] 2>/dev/null || { printf ''; return; }
  if   [ "$d" -ge 86400 ]; then printf '%dd' $(( d / 86400 ))
  elif [ "$d" -ge 3600 ];  then printf '%dh' $(( d / 3600 ))
  else printf '%dm' $(( d / 60 )); fi
}

# trend_note <key> <label> <pct> — %/hr change since the last sample of this
# exact meter, so a limit climbing fast is visible before it's the one
# that's already maxed out. State is one small file per (key, label); blank
# until two samples exist, and blank again once the gap between them is too
# short to be meaningful or so long (>1 day) it would average across noise.
trend_note() {
  local key label pct f now note prev pepoch ppct dt tmp
  key=$1; label=$2; pct=$3
  f="$TREND/$(printf '%s-%s' "$key" "$label" | tr -c '[:alnum:]._-' '-')"
  now=$(date +%s)
  note=""
  if [ -f "$f" ]; then
    prev=$(cat "$f")
    pepoch=${prev%%:*}; ppct=${prev#*:}
    dt=$(( now - ${pepoch:-0} ))
    if [ "$dt" -ge 300 ] && [ "$dt" -le 86400 ] 2>/dev/null; then
      note=$(awk -v p="$pct" -v pp="${ppct:-0}" -v dt="$dt" 'BEGIN {
        rate = (p - pp) * 3600 / dt
        r = rate < 0 ? -rate : rate
        if (r >= 0.5) printf "%s%.1f%%/hr", (rate < 0 ? "-" : "+"), r
      }')
    fi
  fi
  tmp="$f.tmp.$$"
  printf '%s:%s' "$now" "$pct" > "$tmp" && mv "$tmp" "$f" \
    || { rm -f "$tmp"; return 1; }
  printf '%s' "$note"
}

# usage_rows <name> <accent> "<meta|plan|summary|hardstop + window rows>" [stale tag] [trend key]
# Names arrive with a "▸ "/"  " active-login marker. Every available limit
# gets its own meter, so a safe short window cannot hide a tight weekly one.
usage_rows() {
  local name accent encoded stale key meta plan summary hardstop windows max count d
  local label pct reset detail remaining w event rel trend
  name=$1; accent=$2; encoded=$3; stale=$4; key=$5
  meta=$(printf '%s\n' "$encoded" | sed -n '1p')
  plan=$(printf '%s\n' "$meta" | cut -d'|' -f2)
  summary=$(printf '%s\n' "$meta" | cut -d'|' -f3)
  hardstop=$(printf '%s\n' "$meta" | cut -d'|' -f4)
  [ "$summary" = "$meta" ] && summary=""
  windows=$(printf '%s\n' "$encoded" | sed '1d' | awk -F'|' 'NF >= 2 && $2 ~ /^[0-9]+$/')
  [ -z "$windows" ] && { echo "bad|$name|no usage data"; return; }
  max=$(printf '%s\n' "$windows" | awk -F'|' 'BEGIN { m = 0 } $2 > m { m = $2 } END { print m }')
  if [ -z "$summary" ]; then
    count=$(printf '%s\n' "$windows" | awk 'END { print NR }')
    if [ "$count" = 1 ]; then summary="1 limit"; else summary="$count limits"; fi
  fi
  case "$name" in "▸ "*) summary="$summary · active";; esac
  d=$(dot "$max")
  # A provider-reported hard stop (limit actually reached, or critical
  # severity) outranks our own percent-based guess — trust it over dot().
  [ -n "$hardstop" ] && d=bad
  # Numbers nobody can refresh are worth less than the room they take: say so
  # on the row rather than letting a dead login read as a healthy one.
  [ -n "$stale" ] && { summary="$summary · $stale"; [ "$d" = ok ] && d=warn; }
  echo "$d|$name|${plan:+$plan · }$summary"
  printf '%s\n' "$windows" | while IFS='|' read -r label pct reset detail; do
    remaining="$(( 100 - pct ))% left"
    detail="${detail:+$remaining · $detail}"
    [ -n "$detail" ] || detail=$remaining
    if [ -n "$reset" ] && w=$(when_full "$reset"); then
      case "$label" in
        "monthly credits"|"monthly spend") event="billing" ;;
        *) event="reset" ;;
      esac
      rel=$(until_str "$reset")
      if [ -n "$rel" ]; then
        detail="$detail · $event in $rel · $w"
      else
        detail="$detail · $event passed · $w"
      fi
    fi
    if [ -n "$key" ]; then
      trend=$(trend_note "$key" "$label" "$pct")
      [ -n "$trend" ] && detail="${detail:+$detail · }$trend"
    fi
    echo "bar|$accent|$label|$pct|$detail"
  done
}

# is_hidden <service> <name> — accounts.conf turns the account off with a
# third word, `<service> <name> hidden`. A hidden account is never fetched
# or shown; delete the word to bring it back. Nothing is deleted.
is_hidden() {
  local s n flag
  [ -f "$DIR/accounts.conf" ] || return 1
  while read -r s n flag _; do
    [ "$s" = "$1" ] && [ "$n" = "$2" ] && [ "$flag" = hidden ] && return 0
  done < "$DIR/accounts.conf"
  return 1
}

# roster_rows <service> <shown names...> — accounts.conf entries with no
# matching live row appear dormant, so known accounts are never invisible.
roster_rows() {
  local svc s n flag
  svc=$1; shift
  [ -f "$DIR/accounts.conf" ] || return 0
  while read -r s n flag _; do
    [ "$s" = "$svc" ] && [ "$flag" != hidden ] || continue
    case " $* " in *" $n "*) ;; *) echo "off|  $n|not logged in";; esac
  done < "$DIR/accounts.conf"
}

# ---------------------------------------------------------------- codex ----

codex_call() { # codex_call <access_token> <account_id> — 2 on a 401
  if [ -n "$2" ]; then
    headers "Authorization: Bearer $1" "chatgpt-account-id: $2"
  else
    headers "Authorization: Bearer $1"
  fi | http -K - https://chatgpt.com/backend-api/wham/usage
}

# jwt_exp <jwt> — the token's `exp` claim (epoch seconds), blank if unreadable.
jwt_exp() {
  local p
  p=$(printf '%s' "$1" | cut -d. -f2 | tr '_-' '/+')
  case $(( ${#p} % 4 )) in 2) p="$p==" ;; 3) p="$p=" ;; esac
  printf '%s' "$p" | base64 -D 2>/dev/null | jq -r '.exp // empty' 2>/dev/null
}

# Applied to `[auth.json, token response]` — the codex CLI's own merge.
CODEX_MERGE='.[1] as $r | .[0]
  | .tokens.access_token = $r.access_token
  | (if $r.refresh_token then .tokens.refresh_token = $r.refresh_token else . end)
  | (if $r.id_token then .tokens.id_token = $r.id_token else . end)
  | .last_refresh = (now | todate)'

codex_fetch() { # codex_fetch <auth.json path>
  local f=$1 tok acct exp rc
  tok=$(jq -r '.tokens.access_token // empty' "$f") || return 1
  acct=$(jq -r '.tokens.account_id // empty' "$f")
  [ -n "$tok" ] || return 1
  exp=$(jwt_exp "$tok")
  # An undecodable exp isn't proof of expiry; let the server say 401.
  if ! { [ -n "$exp" ] && [ "$exp" -le "$(date +%s)" ] 2>/dev/null; }; then
    codex_call "$tok" "$acct"; rc=$?
    [ "$rc" = 2 ] || return "$rc"
  fi
  with_lock "refresh-codex-$(basename "$(dirname "$f")")-$(basename "$f" .json)" \
    codex_refresh "$f" "$tok"
}

# codex_refresh <auth.json path> <refused access token> — under the lock.
codex_refresh() {
  local f=$1 tok acct resp tmp rc
  tok=$(jq -r '.tokens.access_token // empty' "$f")
  acct=$(jq -r '.tokens.account_id // empty' "$f")
  # Another run (or the CLI) may have refreshed while this one waited.
  if [ -n "$tok" ] && [ "$tok" != "$2" ]; then
    codex_call "$tok" "$acct"; rc=$?
    [ "$rc" = 2 ] || return "$rc"
  fi
  jq -e '.tokens.refresh_token // empty' "$f" >/dev/null || return 1
  # Refresh the same way the codex CLI does; the body is built from the file
  # by jq and handed to curl on stdin, so the refresh token never hits argv.
  resp=$(jq -c --arg c "$CODEX_CLIENT_ID" '
      {client_id: $c, grant_type: "refresh_token",
         refresh_token: .tokens.refresh_token, scope: "openid profile email"}' "$f" \
    | http -X POST -H 'Content-Type: application/json' --data-binary @- \
        https://auth.openai.com/oauth/token) || return 1
  tok=$(printf '%s' "$resp" | jq -r '.access_token // empty')
  [ -n "$tok" ] || return 1
  tmp="$f.tmp.$$"
  { cat "$f"; printf '%s' "$resp"; } | jq -s "$CODEX_MERGE" > "$tmp" \
    && jq -e .tokens.access_token "$tmp" >/dev/null && mv "$tmp" "$f" && chmod 600 "$f" || rm -f "$tmp"
  codex_call "$tok" "$acct"
}

codex_render() { # usage json on stdin -> meta row + one row per limit window
  jq -r '
    def lbl(s): if s == null then "window" elif s <= 21600 then "5h window" elif s >= 518400 then "weekly" else "\(s/3600|floor)h window" end;
    def clampPct: if . > 100 then 100 elif . < 0 then 0 else . end;
    def compact:
      tonumber | round as $n
      | if $n >= 1000 and ($n % 1000) == 0 then "\($n / 1000)k" else "\($n)" end;
    def windows(prefix; rl):
      [rl.primary_window, rl.secondary_window]
      | map(select(. != null) | {
          p: ((.used_percent // 0) | round | clampPct),
          l: (if prefix == "" then lbl(.limit_window_seconds)
              else "\(prefix) \(lbl(.limit_window_seconds))" end),
          r: (.reset_at // null),
          hit: (.limit_reached // false)
        });
    . as $root
    | windows("" ; ($root.rate_limit // {})) as $base
    # code_review_rate_limit mirrors rate_limit but tracks `codex review`
    # separately — it was fetched and discarded before, silently hiding a
    # full review quota behind a healthy-looking main one.
    | windows("review"; ($root.code_review_rate_limit // {})) as $review
    | [($root.additional_rate_limits // [])[]
        | (.limit_name // "model") as $limit_name
        | select(($limit_name | ascii_downcase | contains("spark")) | not)
        | ($limit_name | split("-") | last) as $name
        | windows($name; (.rate_limit // {}))[]] as $extra
    | ($base + $review + $extra) as $all
    | ($root.credits // {}) as $credits
    | ($root.spend_control.individual_limit // null) as $spend
    | (
        if $spend != null then
          "\($spend.remaining | compact) of \($spend.limit | compact) credits left"
        elif ($credits.has_credits // false) and ($credits.balance != null) then
          "\($credits.balance | tonumber | floor) credits left"
        elif ($credits.has_credits // false) and (($credits.approx_local_messages // null) != null) then
          # Message-metered plans have no dollar balance; the local/cloud
          # message estimate is the only human number they do get.
          "~\($credits.approx_local_messages[0])-\($credits.approx_local_messages[1]) msgs left"
        else "" end
      ) as $credit_note
    # A percent can lag reality; these flags mean requests are being
    # rejected right now, so they outrank any percent-based coloring.
    | ( ($credits.overage_limit_reached // false)
        or (($root.rate_limit_reached_type // null) != null)
        or ([$all[] | .hit] | any)
      ) as $hardstop
    | (if $hardstop then
         (if ($credit_note | length) > 0 then "\($credit_note) · limit reached" else "limit reached" end)
       else $credit_note end) as $summary
    | "meta|\($root.plan_type // "")|\($summary)|\(if $hardstop then "1" else "" end)",
      ($all[] | "\(.l)|\(.p)|\(.r // "")|"),
      (if $spend != null then
         "monthly credits|\(($spend.used_percent | tonumber) | round | clampPct)|\($spend.reset_at // "")|"
       else empty end)'
}

codex_rows() {
  local cur cur_id seen_ids names f name id m src ttl retry mirror cache_account out
  cur="${CODEX_HOME:-$HOME/.codex}/auth.json"
  cur_id=$(jq -r '.tokens.account_id // empty' "$cur" 2>/dev/null)
  seen_ids=""; names=""
  for f in "$DIR"/codex/*.json; do
    [ -f "$f" ] || continue
    name=$(basename "$f" .json)
    id=$(jq -r '.tokens.account_id // empty' "$f")
    seen_ids="$seen_ids $id"; names="$names $name"
    is_hidden codex "$name" && continue
    # Refresh tokens rotate: the moment the codex CLI refreshes, the copy
    # captured here is invalidated and this account freezes on its last good
    # reading. So for whichever account is logged in now, read the live
    # login — the one the CLI keeps alive — and mirror it back afterwards so
    # the snapshot is current the day the CLI moves to another account.
    m=" "; src=$f; ttl=$SNAPSHOT_TTL; retry=$SNAPSHOT_RETRY
    [ -n "$cur_id" ] && [ "$id" = "$cur_id" ] && {
      m="▸"; src=$cur; ttl=$LIVE_TTL; retry=$LIVE_RETRY;
    }
    usage_rows "$m $name" teal \
      "$(fetch "codex-$name" "$ttl" "$retry" codex_fetch "$src" | codex_render)" "$(staleness "codex-$name")" "codex-$name"
    if [ "$src" = "$cur" ]; then
      mirror="$f.tmp.$$"
      cp "$cur" "$mirror" 2>/dev/null && chmod 600 "$mirror" && mv "$mirror" "$f" \
        || rm -f "$mirror"
    fi
  done
  while :; do
    [ -f "$cur" ] || break
    if [ -n "$cur_id" ]; then
      case " $seen_ids " in *" $cur_id "*) break;; esac
    fi
    cache_account=$(printf '%s' "${cur_id:-current}" | tr -c '[:alnum:]._-' '-')
    out=$(fetch "codex-current-$cache_account" "$LIVE_TTL" "$LIVE_RETRY" codex_fetch "$cur")
    [ -n "$out" ] || break
    name=$(printf '%s\n' "$out" | jq -r '.email // "" | split("@")[0]')
    names="$names ${name:-current}"
    is_hidden codex "${name:-current}" && break
    usage_rows "▸ ${name:-current}" teal \
      "$(printf '%s\n' "$out" | codex_render)" "$(staleness "codex-current-$cache_account")" "codex-current-$cache_account"
    break
  done
  roster_rows codex $names
}

# --------------------------------------------------------------- claude ----

claude_desktop_account() {
  jq -r '.lastKnownAccountUuid // empty' \
    "$HOME/Library/Application Support/Claude/config.json" 2>/dev/null
}

claude_cli_account() {
  jq -r '.oauthAccount.accountUuid // empty' "$HOME/.claude.json" 2>/dev/null
}

claude_valid_uuid() {
  printf '%s\n' "$1" \
    | grep -Eq '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
}

claude_safe_storage_key() {
  local safe
  safe=$(security find-generic-password -s "Claude Safe Storage" -w 2>/dev/null) || return 1
  # openssl takes the passphrase (and below, the AES key) only as an argument;
  # it's a short-lived local process, and the key alone grants no API access.
  openssl kdf -keylen 16 -kdfopt digest:SHA1 \
    -kdfopt "pass:$safe" -kdfopt salt:saltysalt -kdfopt iter:1003 \
    PBKDF2 2>/dev/null | tr -d ':\n'
}

# Claude Desktop passes the active account's OAuth token to each embedded
# Claude Code process. Newer app releases keep the web session cookie and,
# when safeStorage is unavailable, even the refreshed OAuth token in memory,
# so the running child is the only live credential visible outside Electron.
claude_desktop_process_token() {
  local account pids pid line tok
  account=$(claude_desktop_account)
  [ -n "$account" ] || return 1
  pids=$(ps -awwxo pid=,command= 2>/dev/null | awk -v account="$account" '
    index($0, "/Library/Application Support/Claude/claude-code/") && index($0, "/claude.app/Contents/MacOS/claude") && index($0, account) {
      print $1
    }
  ')
  for pid in $pids; do
    line=$(ps eww -p "$pid" -o command= 2>/dev/null) || continue
    tok=$(printf '%s\n' "$line" \
      | sed -n 's/.*[[:space:]]CLAUDE_CODE_OAUTH_TOKEN=\([^[:space:]]*\).*/\1/p')
    [ -n "$tok" ] && { printf '%s' "$tok"; return 0; }
  done
  return 1
}

# When Electron can use safeStorage it persists scoped Desktop OAuth tokens
# in config.json. Read only a non-expired inference token for the active
# account; never copy it into the insomnia cache.
claude_desktop_cached_token() {
  local base account key nowms field enc plain tok
  base="$HOME/Library/Application Support/Claude"
  account=$(claude_desktop_account)
  [ -n "$account" ] && [ -f "$base/config.json" ] || return 1
  key=$(claude_safe_storage_key) || return 1
  nowms=$(( $(date +%s) * 1000 ))
  for field in 'oauth:tokenCacheV2' 'oauth:tokenCache'; do
    enc=$(jq -r --arg field "$field" '.[$field] // empty' "$base/config.json" 2>/dev/null)
    [ -n "$enc" ] || continue
    plain=$(printf '%s' "$enc" | base64 -D 2>/dev/null | tail -c +4 \
      | openssl enc -d -aes-128-cbc -K "$key" \
          -iv 20202020202020202020202020202020 2>/dev/null) || continue
    tok=$(printf '%s' "$plain" | jq -r \
      --arg prefix "acct:$account|" --argjson now "$nowms" '
        [to_entries[]
          | select(.key | startswith($prefix))
          | select(.key | contains("user:inference"))
          | .value
          | select((.expiresAt // 0) > $now)
          | .token // empty]
        | first // empty' 2>/dev/null)
    [ -n "$tok" ] && { printf '%s' "$tok"; return 0; }
  done
  return 1
}

claude_desktop_org() {
  local base account org
  base="$HOME/Library/Application Support/Claude"
  account=$(claude_desktop_account)
  [ -n "$account" ] || return 1

  org=$(ps -awwxo command= 2>/dev/null | awk -v account="$account" '
    index($0, "/claude.app/Contents/MacOS/claude") && index($0, account) {
      count = split($0, fields, /[[:space:]]+/)
      for (i = 1; i <= count; i++) {
        if (!index(fields[i], "/skills-plugin/")) continue
        parts_count = split(fields[i], parts, "/")
        for (j = 1; j <= parts_count - 2; j++) {
          if (parts[j] == "skills-plugin" && parts[j + 2] == account) {
            print parts[j + 1]
            exit
          }
        }
      }
    }
  ')
  if claude_valid_uuid "$org"; then printf '%s' "$org"; return 0; fi

  org=$(jq -r '[.samples[]? | select(.org != null)] | .[-1].org // empty' \
    "$base/plan-usage-history.json" 2>/dev/null)
  if claude_valid_uuid "$org"; then printf '%s' "$org"; return 0; fi

  if [ "$account" = "$(claude_cli_account)" ]; then
    org=$(jq -r '.oauthAccount.organizationUuid // empty' "$HOME/.claude.json" 2>/dev/null)
    if claude_valid_uuid "$org"; then printf '%s' "$org"; return 0; fi
  fi

  org=$(find "$base/local-agent-mode-sessions/$account" \
    -mindepth 1 -maxdepth 1 -type d -exec stat -f '%m %N' {} \; 2>/dev/null \
    | sort -nr | sed -n '1s#.*/##p')
  claude_valid_uuid "$org" || return 1
  printf '%s' "$org"
}

# Older Claude Desktop releases persisted the claude.ai session in Chromium's
# cookie store. Keep that route as a fallback for machines where no scoped
# OAuth token is available. Cookie material lives only in this temporary dir.
claude_desktop_cookie_fetch() (
  base="$HOME/Library/Application Support/Claude"
  [ -f "$base/Cookies" ] && [ -f "$base/config.json" ] || exit 1
  command -v security >/dev/null && command -v sqlite3 >/dev/null \
    && command -v openssl >/dev/null && command -v xxd >/dev/null || exit 1

  work=$(mktemp -d "${TMPDIR:-/tmp}/insomnia-claude-usage.XXXXXX") || exit 1
  cleanup() {
    local name
    for name in sessionKey sessionKeyV2 sessionKeyV3 sessionKeyLC sessionKeyV3LC lastActiveOrg; do
      rm -f "$work/$name.raw" "$work/$name"
    done
    rm -f "$work/cookies" "$work/usage.json"
    rmdir "$work" 2>/dev/null
  }
  trap cleanup EXIT HUP INT TERM

  key=$(claude_safe_storage_key)
  [ -n "$key" ] || exit 1

  read_cookie() {
    local name=$1 enc
    case "$name" in
      sessionKey|sessionKeyV2|sessionKeyV3|sessionKeyLC|sessionKeyV3LC|lastActiveOrg) ;;
      *) return 1 ;;
    esac
    enc=$(sqlite3 "$base/Cookies" \
      "select hex(encrypted_value) from cookies
       where host_key in ('.claude.ai','claude.ai','.claude.com','claude.com')
         and name='$name'
       order by expires_utc desc limit 1;" \
      2>/dev/null)
    [ -n "$enc" ] || return 1
    printf '%s' "$enc" | xxd -r -p | tail -c +4 \
      | openssl enc -d -aes-128-cbc -K "$key" \
          -iv 20202020202020202020202020202020 \
          > "$work/$name.raw" 2>/dev/null || return 1

    if LC_ALL=C grep -aq '^sk-ant-' "$work/$name.raw" \
      || LC_ALL=C grep -aqE '^[0-9a-fA-F-]{36}$' "$work/$name.raw"; then
      cp "$work/$name.raw" "$work/$name"
    else
      # Current Chromium prefixes encrypted cookies with SHA-256(host).
      tail -c +33 "$work/$name.raw" > "$work/$name"
    fi
  }

  found_session=0
  for name in sessionKey sessionKeyV2 sessionKeyV3 sessionKeyLC sessionKeyV3LC; do
    read_cookie "$name" || continue
    case "$(cat "$work/$name")" in sk-ant-sid*) found_session=1;; esac
  done
  [ "$found_session" = 1 ] || exit 1
  read_cookie lastActiveOrg || true

  org=$(cat "$work/lastActiveOrg" 2>/dev/null)
  claude_valid_uuid "$org" || org=$(claude_desktop_org)
  claude_valid_uuid "$org" || exit 1

  {
    printf '# Netscape HTTP Cookie File\n'
    for name in sessionKey sessionKeyV2 sessionKeyV3 sessionKeyLC sessionKeyV3LC; do
      [ -s "$work/$name" ] \
        && printf '.claude.ai\tTRUE\t/\tTRUE\t0\t%s\t%s\n' "$name" "$(cat "$work/$name")"
    done
    printf '.claude.ai\tTRUE\t/\tTRUE\t0\tlastActiveOrg\t%s\n' "$org"
  } > "$work/cookies"
  chmod 600 "$work/cookies"

  $CURL --http1.1 --compressed -o "$work/usage.json" \
    -b "$work/cookies" \
    -H "Accept: application/json" \
    -H "Referer: https://claude.ai/" \
    -H "Origin: https://claude.ai" \
    -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/138 Safari/537.36" \
    "https://claude.ai/api/organizations/$org/usage" \
    && jq -e 'type == "object"' "$work/usage.json" >/dev/null \
    && cat "$work/usage.json"
)

claude_desktop_fetch() {
  local tok
  tok=$(claude_desktop_process_token) || tok=""
  [ -n "$tok" ] && { claude_usage_call "$tok" && return 0; }
  tok=$(claude_desktop_cached_token) || tok=""
  [ -n "$tok" ] && { claude_usage_call "$tok" && return 0; }
  claude_desktop_cookie_fetch
}

claude_usage_call() { # claude_usage_call <access_token> — 2 on a 401
  headers "Authorization: Bearer $1" "anthropic-beta: oauth-2025-04-20" \
    | http -K - https://api.anthropic.com/api/oauth/usage
}

claude_keychain() {
  security find-generic-password -s "Claude Code-credentials" -w 2>/dev/null
}

# claude_expired <expiresAt ms> — anything but a future expiry counts.
claude_expired() {
  ! [ "${1:-0}" -gt "$(( $(date +%s) * 1000 ))" ] 2>/dev/null
}

# claude_refresh <refresh_token> — the token response from whichever OAuth
# host answers. The body reaches curl on stdin, never argv.
claude_refresh() {
  local body host
  body=$(printf '{"grant_type":"refresh_token","refresh_token":"%s","client_id":"%s"}' \
    "$1" "$CLAUDE_CLIENT_ID")
  for host in platform.claude.com console.anthropic.com; do
    printf '%s' "$body" \
      | http -X POST -H 'Content-Type: application/json' --data-binary @- \
          "https://$host/v1/oauth/token" \
      && return 0
  done
  return 1
}

# Applied to `[credentials, token response]`; only where the result is
# persisted (Keychain or snapshot file) differs between callers.
CLAUDE_MERGE='.[1] as $r | .[0]
  | .claudeAiOauth.accessToken = $r.access_token
  | (if $r.refresh_token then .claudeAiOauth.refreshToken = $r.refresh_token else . end)
  | .claudeAiOauth.expiresAt = ((now + ($r.expires_in // 3600)) * 1000 | floor)'

claude_fetch_current() {
  local desktop_account cred tok rc
  desktop_account=$(claude_desktop_account)
  if [ -n "$desktop_account" ] && [ "$desktop_account" = "$(claude_cli_account)" ]; then
    claude_desktop_fetch && return 0
  fi

  # The desktop session was unavailable, so fall back to Claude Code's
  # Keychain token.
  cred=$(claude_keychain) || return 1
  tok=$(printf '%s' "$cred" | jq -r '.claudeAiOauth.accessToken // empty')
  if [ -n "$tok" ] \
    && ! claude_expired "$(printf '%s' "$cred" | jq -r '.claudeAiOauth.expiresAt // 0')"; then
    claude_usage_call "$tok"; rc=$?
    # A network/provider failure with a nominally valid token is not evidence
    # that it should be rotated. Let fetch() serve the marked stale cache.
    [ "$rc" = 2 ] || return "$rc"
  fi
  with_lock refresh-claude-keychain claude_refresh_keychain "$tok"
}

# claude_refresh_keychain <refused access token> — under the lock. Renews the
# Keychain item itself so both this card and the next CLI invocation share
# the rotated credentials.
claude_refresh_keychain() {
  local cred tok rt resp updated rc
  cred=$(claude_keychain) || return 1
  tok=$(printf '%s' "$cred" | jq -r '.claudeAiOauth.accessToken // empty')
  # Another run (or Claude Code) may have refreshed while this one waited.
  if [ -n "$tok" ] && [ "$tok" != "$1" ]; then
    claude_usage_call "$tok"; rc=$?
    [ "$rc" = 2 ] || return "$rc"
  fi
  rt=$(printf '%s' "$cred" | jq -r '.claudeAiOauth.refreshToken // empty')
  [ -n "$rt" ] || return 1
  resp=$(claude_refresh "$rt") || return 1
  tok=$(printf '%s' "$resp" | jq -r '.access_token // empty')
  [ -n "$tok" ] || return 1
  updated=$(printf '%s\n%s' "$cred" "$resp" | jq -s "$CLAUDE_MERGE") || return 1
  [ -n "$updated" ] || return 1
  # `security` takes a password only as an argument or from a tty prompt.
  security add-generic-password -U -a "$(id -un)" -s "Claude Code-credentials" -w "$updated" >/dev/null \
    || return 1
  claude_usage_call "$tok"
}

claude_snapshot() { # claude_snapshot <path> — write the live keychain login there
  local cred email account org tmp
  cred=$(claude_keychain) || return 1
  email=$(jq -r '.oauthAccount.emailAddress // empty' "$HOME/.claude.json" 2>/dev/null)
  account=$(claude_cli_account)
  org=$(jq -r '.oauthAccount.organizationUuid // empty' "$HOME/.claude.json" 2>/dev/null)
  tmp="$1.tmp.$$"
  printf '%s' "$cred" | jq --arg e "$email" --arg a "$account" --arg o "$org" \
    '. + {email: $e, account_uuid: $a, organization_uuid: $o}' > "$tmp" \
    && jq -e .claudeAiOauth.accessToken "$tmp" >/dev/null \
    && mv "$tmp" "$1" && chmod 600 "$1" || { rm -f "$tmp"; return 1; }
}

claude_fetch_file() { # claude_fetch_file <snapshot path>
  local f=$1 tok rc
  tok=$(jq -r '.claudeAiOauth.accessToken // empty' "$f")
  if [ -n "$tok" ] \
    && ! claude_expired "$(jq -r '.claudeAiOauth.expiresAt // 0' "$f")"; then
    claude_usage_call "$tok"; rc=$?
    [ "$rc" = 2 ] || return "$rc"
  fi
  with_lock "refresh-claude-$(basename "$f" .json)" claude_refresh_file "$f" "$tok"
}

# claude_refresh_file <snapshot path> <refused access token> — under the lock.
claude_refresh_file() {
  local f=$1 tok rt resp tmp rc
  tok=$(jq -r '.claudeAiOauth.accessToken // empty' "$f")
  if [ -n "$tok" ] && [ "$tok" != "$2" ]; then
    claude_usage_call "$tok"; rc=$?
    [ "$rc" = 2 ] || return "$rc"
  fi
  rt=$(jq -r '.claudeAiOauth.refreshToken // empty' "$f")
  [ -n "$rt" ] || return 1
  resp=$(claude_refresh "$rt") || return 1
  tok=$(printf '%s' "$resp" | jq -r '.access_token // empty')
  [ -n "$tok" ] || return 1
  tmp="$f.tmp.$$"
  { cat "$f"; printf '%s' "$resp"; } | jq -s "$CLAUDE_MERGE" > "$tmp" \
    && jq -e .claudeAiOauth.accessToken "$tmp" >/dev/null && mv "$tmp" "$f" && chmod 600 "$f" || rm -f "$tmp"
  claude_usage_call "$tok"
}

claude_render() { # usage json on stdin -> meta row + one row per limit window
  jq -r '
    def ep(s): if s == null then null else (s | sub("\\.[0-9]+"; "") | sub("\\+00:00$"; "Z") | try fromdateiso8601 catch null) end;
    def clampPct: if . > 100 then 100 elif . < 0 then 0 else . end;
    def scopedName:
      (.scope.model.display_name // .scope.model.id
       // .scope.surface.display_name // .scope.surface.id
       // .kind // "scoped");
    def money:
      (.amount_minor // 0) as $n
      | (.exponent // 2) as $e
      | if $e == 0 then "\($n)"
        elif $e == 1 then "\($n / 10)"
        else "\($n / 100)"
        end;
    . as $root
    | [
        {l: "5h window", w: .five_hour},
        {l: "weekly", w: .seven_day},
        {l: "Opus weekly", w: .seven_day_opus},
        {l: "Sonnet weekly", w: .seven_day_sonnet},
        {l: "Cowork weekly", w: .seven_day_cowork},
        {l: "OAuth apps weekly", w: .seven_day_oauth_apps}
      ]
      | map(select(.w != null) | {l, p: ((.w.utilization // 0) | round | clampPct), r: ep(.w.resets_at)}) as $windows
    # Scoped limits are provider-named and therefore pick up new models and
    # versions, including Fable 5.1, without another hard-coded field.
    | [($root.limits // [])[]
        | select(.scope != null)
        | {
            l: ((scopedName
                + (if .group == "weekly" then " weekly"
                   elif .group == "session" then " session" else "" end))),
            p: ((.percent // 0) | round | clampPct),
            r: ep(.resets_at)
          }] as $scoped
    | ($scoped | map(.l)) as $scoped_labels
    | (($windows | map(select(.l as $l | ($scoped_labels | index($l) | not)))) + $scoped) as $all_windows
    # Claude Desktop now names overage billing `extra_usage`; the OAuth
    # endpoint currently returns both shapes, but normalize the new one first
    # so the card stays correct when the legacy `spend` object disappears.
    | ($root.extra_usage // {}) as $extra
    | ($root.spend // {}) as $legacy_spend
    | (($extra.is_enabled // false) and ($extra.monthly_limit != null)) as $has_extra
    | (($legacy_spend.enabled // false)
        and ($legacy_spend.limit.amount_minor != null)) as $has_legacy_spend
    | (if $has_extra then
         {
           enabled: true,
           percent: ($extra.utilization
             // (if ($extra.monthly_limit // 0) > 0
                 then (($extra.used_credits // 0) * 100 / $extra.monthly_limit)
                 else 0 end)),
           severity: (if ($extra.spend_limit_reached // false)
             then "critical" else "normal" end),
           used: {
             amount_minor: ($extra.used_credits // 0),
             exponent: ($extra.decimal_places // 2)
           },
           limit: {
             amount_minor: ($extra.monthly_limit // 0),
             exponent: ($extra.decimal_places // 2)
           }
         }
       else $legacy_spend end) as $spend
    | ($has_extra or $has_legacy_spend) as $has_spend
    | ($spend.used | money) as $used
    | ($spend.limit | money) as $limit
    | (($spend.limit.amount_minor // 0) - ($spend.used.amount_minor // 0)) as $remaining
    | ($remaining
        | {amount_minor: (if . < 0 then -. else . end), exponent: ($spend.limit.exponent // 2)}
        | money) as $difference
    # member_dashboard_available is an explicit org/workspace-seat signal
    # from the API, rather than guessing "enterprise" purely from the shape
    # (spend cap present, no rate-limit windows) — that shape also matches a
    # personal plan with pay-as-you-go overage maxed out.
    | (($root.member_dashboard_available // false) and $has_spend and ($all_windows | length) == 0) as $is_org
    # severity is the providers own critical/normal judgment per limit, more
    # trustworthy than re-deriving color from a flat percent cutoff.
    | ([(($root.limits // [])[] | .severity // "normal"), ($spend.severity // "normal")]
        | any(. == "critical")) as $hardstop
    | "meta|\(if $is_org then "org" else "" end)|\(if $has_spend then
        "$\($difference) \(if $remaining < 0 then "over" else "left" end)"
        else "" end)|\(if $hardstop then "1" else "" end)",
      ($all_windows[] | "\(.l)|\(.p)|\(.r // "")|"),
      (if $has_spend then
         "monthly spend|\(($spend.percent // 0) | round | clampPct)||spent $\($used) of $\($limit)"
       else empty end)'
}

claude_rows() {
  local cur_email cur_account desktop_account legacy_desktop legacy_count f account em
  local seen seen_accounts names name m live mirror out cache_account
  cur_email=$(jq -r '.oauthAccount.emailAddress // empty' "$HOME/.claude.json" 2>/dev/null)
  cur_account=$(claude_cli_account)
  desktop_account=$(claude_desktop_account)
  legacy_desktop=""; legacy_count=0
  if [ -n "$desktop_account" ] && [ "$desktop_account" != "$cur_account" ]; then
    for f in "$DIR"/claude/*.json; do
      [ -f "$f" ] || continue
      account=$(jq -r '.account_uuid // empty' "$f")
      em=$(jq -r '.email // empty' "$f")
      if [ -z "$account" ] && { [ -z "$cur_email" ] || [ "$em" != "$cur_email" ]; }; then
        legacy_desktop=$f
        legacy_count=$(( legacy_count + 1 ))
      fi
    done
    [ "$legacy_count" = 1 ] || legacy_desktop=""
  fi
  seen=""; seen_accounts=""; names=""
  for f in "$DIR"/claude/*.json; do
    [ -f "$f" ] || continue
    name=$(basename "$f" .json)
    em=$(jq -r '.email // empty' "$f")
    account=$(jq -r '.account_uuid // empty' "$f")
    [ -n "$legacy_desktop" ] && [ "$f" = "$legacy_desktop" ] && account=$desktop_account
    seen="$seen $em"; seen_accounts="$seen_accounts $account"; names="$names $name"
    is_hidden claude "$name" && continue
    # Same rotation trap as codex: once Claude Code refreshes, the captured
    # copy is dead. Read the keychain for the account that's logged in now,
    # then re-snapshot it so this file keeps up.
    m=" "; live=""; mirror=0
    if [ -n "$desktop_account" ] && [ "$account" = "$desktop_account" ]; then
      m="▸"; live=desktop
      [ "$desktop_account" = "$cur_account" ] && mirror=1
    elif { [ -n "$cur_account" ] && [ "$account" = "$cur_account" ]; } \
      || { [ -n "$cur_email" ] && [ "$em" = "$cur_email" ]; }; then
      m="▸"; live=cli; mirror=1
    fi
    if [ "$live" = desktop ]; then
      out=$(fetch "claude-$name" "$LIVE_TTL" "$LIVE_RETRY" claude_desktop_fetch)
    elif [ "$live" = cli ]; then
      out=$(fetch "claude-$name" "$LIVE_TTL" "$LIVE_RETRY" claude_fetch_current)
    else
      out=$(fetch "claude-$name" "$SNAPSHOT_TTL" "$SNAPSHOT_RETRY" claude_fetch_file "$f")
    fi
    usage_rows "$m $name" coral \
      "$(printf '%s\n' "$out" | claude_render)" "$(staleness "claude-$name")" "claude-$name"
    [ "$mirror" = 1 ] && claude_snapshot "$f"
  done
  while :; do
    if [ -n "$cur_email" ]; then
      case " $seen " in *" $cur_email "*) break;; esac
    fi
    name=${cur_email%%@*}; name=${name:-current}
    is_hidden claude "$name" && break
    cache_account=$(printf '%s' "${cur_email:-current}" | tr -c '[:alnum:]._-' '-')
    out=$(fetch "claude-current-$cache_account" "$LIVE_TTL" "$LIVE_RETRY" claude_fetch_current)
    [ -n "$out" ] || break
    names="$names $name"
    usage_rows "▸ $name" coral \
      "$(printf '%s\n' "$out" | claude_render)" "$(staleness "claude-current-$cache_account")" "claude-current-$cache_account"
    break
  done
  if [ -n "$desktop_account" ] && [ "$desktop_account" != "$cur_account" ] \
    && ! is_hidden claude desktop; then
    case " $seen_accounts " in
      *" $desktop_account "*) ;;
      *)
        cache_account=$(printf '%s' "$desktop_account" | tr -c '[:alnum:]._-' '-')
        out=$(fetch "claude-desktop-$cache_account" "$LIVE_TTL" "$LIVE_RETRY" claude_desktop_fetch)
        if [ -n "$out" ]; then
          names="$names desktop"
          usage_rows "▸ desktop" coral \
            "$(printf '%s\n' "$out" | claude_render)" "$(staleness "claude-desktop-$cache_account")" "claude-desktop-$cache_account"
        fi
        ;;
    esac
  fi
  roster_rows claude $names
}

# -------------------------------------------------------------- copilot ----

copilot_fetch() { # copilot_fetch <gh username>
  local tok
  tok=$(gh auth token --user "$1" 2>/dev/null)
  [ -n "$tok" ] || return 1
  headers "Authorization: Bearer $tok" "User-Agent: GitHubCopilotChat/0.26" \
    | http -K - https://api.github.com/copilot_internal/user
}

# copilot_accounts — "<login> <active>" for each working github.com login.
# Newer gh has `auth status --json hosts`; older ones only print prose, so
# scrape that as a fallback.
copilot_accounts() {
  local json
  if json=$(gh auth status --json hosts 2>/dev/null) \
    && printf '%s' "$json" | jq -e '.hosts | type == "object"' >/dev/null 2>&1; then
    printf '%s' "$json" \
      | jq -r '.hosts["github.com"][]? | select(.state == "success") | "\(.login) \(.active)"'
  else
    gh auth status 2>&1 \
      | awk '/Logged in to github.com account/ {u=$7} /Active account:/ {if (u != "") print u, $NF; u=""}'
  fi
}

copilot_rows() {
  local accts names u act m out
  accts=$(copilot_accounts)
  names=""
  while read -r u act; do
    [ -n "$u" ] || continue
    names="$names $u"
    is_hidden copilot "$u" && continue
    m=" "; [ "$act" = "true" ] && m="▸"
    out=$(fetch "copilot-$u" 600 "$SNAPSHOT_RETRY" copilot_fetch "$u")
    [ -n "$out" ] || continue
    printf '%s\n' "$out" | jq -r --arg n "$m $u" --arg s "$(staleness "copilot-$u")" '
      def clampPct: if . > 100 then 100 elif . < 0 then 0 else . end;
      def pctused(x): ((100 - (x.percent_remaining // 100)) | round | clampPct);
      ({business: "biz", enterprise: "ent", individual: "ind"}[.copilot_plan] // .copilot_plan // "?") as $plan
      | (.quota_snapshots.premium_interactions // {}) as $q
      | (.quota_snapshots.chat // {}) as $qc
      | (.quota_snapshots.completions // {}) as $qi
      | (.quota_reset_date // "") as $rst_full
      | ($rst_full | if . == "" then "" else .[5:] end) as $rst
      | (if $rst_full != "" then
           ((($rst_full + "T00:00:00Z") | strptime("%Y-%m-%dT%H:%M:%SZ") | mktime) - now) / 86400 | floor
         else null end) as $days_left
      | (if $rst == "" then "" else
           "resets \($rst)" + (if $days_left != null and $days_left >= 0 then " (in \($days_left)d)" else "" end)
         end) as $reset_note
      | (if $s == "" then "" else " · \($s)" end) as $tag
      # premium_interactions was the only quota read before — chat and
      # completions are real, finite quotas on Individual plans (just
      # unlimited on business/enterprise, where they stayed invisible either way).
      | [
          (if ($q.unlimited // false) then empty else {name: "premium", p: pctused($q)} end),
          (if ($qc.has_quota // false) and (($qc.unlimited // false) | not) then {name: "chat", p: pctused($qc)} else empty end),
          (if ($qi.has_quota // false) and (($qi.unlimited // false) | not) then {name: "completions", p: pctused($qi)} else empty end)
        ] as $bars
      | (if ($bars | length) > 0 then ([$bars[].p] | max) else 0 end) as $worst
      | (($q.unlimited // false) and ($bars | length) == 0) as $all_unlimited
      | [
          (if $all_unlimited then
             "\(if $s == "" then "ok" else "warn" end)|\($n)|\($plan) · unlimited\($tag)"
           else
             (if $worst >= 90 then "bad" elif $worst >= 75 or $s != "" then "warn" else "ok" end)
               + "|\($n)|\($plan) · \($worst)% used\($tag)"
           end),
          ($bars[] | "bar|violet|\(.name)|\(.p)|\($reset_note)")
        ][]'
  done <<EOF
$accts
EOF
  roster_rows copilot $names
}

# -------------------------------------------------------------- capture ----

capture() {
  local svc=$1 name=$2 src email
  [ -n "$svc" ] && [ -n "$name" ] || { echo "usage: ai-usage.sh capture <codex|claude> <name>" >&2; exit 1; }
  # The name becomes a file name and a protocol field: no paths, no '|'.
  # Spelled out rather than A-Z, which some locales collate loosely.
  case $name in
    .*|*[!abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-]*)
      echo "invalid name '$name': use letters, digits, '.', '_' and '-', not starting with '.'" >&2
      exit 1 ;;
  esac
  case $svc in
    codex)
      src="${CODEX_HOME:-$HOME/.codex}/auth.json"
      [ -f "$src" ] || { echo "no codex login found at $src" >&2; exit 1; }
      cp "$src" "$DIR/codex/$name.json" && chmod 600 "$DIR/codex/$name.json"
      echo "captured codex login as '$name' ($(jq -r '.tokens.account_id // "?"' "$src"))" ;;
    claude)
      claude_snapshot "$DIR/claude/$name.json" \
        || { echo "no Claude Code keychain credentials found" >&2; exit 1; }
      email=$(jq -r '.oauthAccount.emailAddress // empty' "$HOME/.claude.json" 2>/dev/null)
      echo "captured claude login as '$name' (${email:-unknown email})" ;;
    *) echo "capture supports: codex, claude (copilot uses 'gh auth login' accounts)" >&2; exit 1 ;;
  esac
}

# rollup_line <encoded rows on stdin> — names the single tightest meter
# across every service/account so it doesn't take reading every section to
# spot it. Silent unless something is genuinely close to its limit (>=90%,
# the same cutoff `dot()` already uses for the red state).
rollup_line() {
  awk -F'|' '
    $1 == "hdr" { section = $3; next }
    $1 == "ok" || $1 == "warn" || $1 == "bad" || $1 == "off" {
      acct = $2; gsub(/^[ \t]*[▸ ]*/, "", acct); next
    }
    $1 == "bar" && $4 ~ /^[0-9]+$/ {
      pct = $4 + 0
      if (pct > best) { best = pct; bestlabel = $3; bestacct = acct; bestsection = section }
    }
    END {
      if (best + 0 >= 90) {
        printf "bad|⚠ nearest limit|%s %s · %s %d%%\n", bestsection, bestacct, bestlabel, best
      }
    }
  '
}

case ${1:-} in
  all)     body=$(
             c=$(claude_rows);  [ -n "$c" ] && { echo "hdr|coral|✻ claude"; printf '%s\n' "$c"; }
             o=$(codex_rows);   [ -n "$o" ] && { echo "hdr|teal|◎ openai codex"; printf '%s\n' "$o"; }
             p=$(copilot_rows); [ -n "$p" ] && { echo "hdr|violet|⧉ copilot"; printf '%s\n' "$p"; }
           )
           top=$(printf '%s\n' "$body" | rollup_line)
           [ -n "$top" ] && printf '%s\n' "$top"
           printf '%s\n' "$body"
           exit 0 ;;
  codex)   codex_rows ;;
  claude)  claude_rows ;;
  copilot) copilot_rows ;;
  capture) capture "${2:-}" "${3:-}" ;;
  *) echo "usage: ai-usage.sh <all|codex|claude|copilot|capture>" >&2; exit 1 ;;
esac
