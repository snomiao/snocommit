#!/bin/sh
# Every proof that the binary behaves like the TypeScript, in one go, on
# Linux, macOS or Windows (in Git Bash there):
#
#   sh rs/parity/proof.sh <rustify's templates/parity/parity.mjs> <the snocommit binary>
#
# from the repository root, after `bun install` and `bun run build` (the
# commands' JavaScript, dist/cli-<command>.js, is a build product). The CI
# workflow's parity job runs this on each platform of `parity-platforms`,
# against the binary it has just built for that platform ($RUSTIFY_PLATFORM
# names it; without it, `uname` does).
#
# First rs/parity.json is checked to be what rs/parity.gen.mjs writes. The
# binary is put under the six commands' names into a directory of its own
# (links; copies on Windows), since it takes its commit type from the name it
# was started as. Then rs/parity.json is split into lanes
# (rs/parity/lanes.mjs), which run side by side, each into a log of its own,
# every lane:
#
#   - TypeScript against the binary (SNOCOMMIT_PACKAGE_ROOT lets the binary
#     hand back to the real dist/cli-<command>.js what it does not answer),
#     both through rs/parity/with-mock.mjs, the stand-in for the OpenAI API;
#   - on Linux and macOS the same again with stdout and stderr a terminal
#     (--tty stdout,stderr; stdin stays a pipe), for every case that does
#     not say otherwise: the echo of the git commands is blue there, and
#     what git and its hooks print reaches the terminal as node passed it on;
#   - then, for each command the lane has cases of, the binary alone against
#     a stand-in package, which fails unless exactly the cases marked
#     "handback" for this system reach the JavaScript.
#
# As many lanes as the machine has processors, three at most on Windows
# (PARITY_LANES says otherwise). On macOS and Windows the crate's own tests
# run beside them (`cargo test`; on Linux the workflow has a job of its own
# for them); it builds the crate again from the same sources and Cargo.lock,
# and does not run the binary named above.
#
# On Windows the binary hands every invocation back to the JavaScript before
# it does anything (rs/src/commit.rs prepare): there the lanes show that every
# case reaches the JavaScript through the binary and behaves as the
# JavaScript started directly does, the terminal cases skipped (no
# pseudo-terminal).
#
# At the end one line is printed for each run that passed, and all of what a
# failed one wrote. The exit code is 0 only when all of them passed.
set -u
usage="usage: proof.sh <parity.mjs> <binary>"
# In Git Bash `pwd -W` gives a directory as Windows programs (node, the
# binary) read it.
here=$(pwd -W 2> /dev/null || pwd)
absolute() {
  case "$1" in
    /* | [A-Za-z]:[\\/]*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "$here" "$1" ;;
  esac
}
runner=$(absolute "${1:?$usage}")
binary=$(absolute "${2:?$usage}")
if command -v cygpath > /dev/null 2>&1; then
  runner=$(cygpath -m "$runner")
  binary=$(cygpath -m "$binary")
fi
cd "$(dirname "$0")/../.." || exit 1
root=$(pwd -W 2> /dev/null || pwd)

platform=${RUSTIFY_PLATFORM:-}
if [ -z "$platform" ]; then
  case "$(uname -s)" in
    Linux) platform=linux ;;
    Darwin) platform=darwin ;;
    MINGW* | MSYS* | CYGWIN*) platform=win32 ;;
    *) platform=$(uname -s) ;;
  esac
fi
processors=$(getconf _NPROCESSORS_ONLN 2> /dev/null || echo "${NUMBER_OF_PROCESSORS:-2}")
case "$platform" in
  win32*) lanes=${PARITY_LANES:-$((processors < 3 ? processors : 3))} others="unit" ;;
  darwin*) lanes=${PARITY_LANES:-$processors} others="unit" ;;
  *) lanes=${PARITY_LANES:-$processors} others="" ;;
esac

node rs/parity.gen.mjs --check || exit 1
for file in dist/cli-chore.js dist/cli-docs.js dist/cli-feat.js dist/cli-fix.js dist/cli-refactor.js dist/cli-styles.js; do
  [ -f "$file" ] || {
    echo "proof: $file is missing: run \`bun run build\` first"
    exit 1
  }
done

# In $TMPDIR, which macOS's mktemp passes over without a template.
logs=$(mktemp -d "${TMPDIR:-/tmp}/snocommit-proof.XXXXXX") || exit 1
logs_for_node=$(cygpath -m "$logs" 2> /dev/null || printf '%s' "$logs")

# The binary under the commands' names.
bin=$logs_for_node/bin
mkdir "$logs/bin" || exit 1
commands="chore docs feat fix refactor styles"
case "$platform" in
  win32*)
    exe=$binary
    [ -f "$exe" ] || exe=$binary.exe
    for name in $commands; do cp "$exe" "$logs/bin/$name.exe" || exit 1; done
    ;;
  *) for name in $commands; do ln -s "$binary" "$logs/bin/$name" || exit 1; done ;;
esac

node rs/parity/lanes.mjs rs/parity.json "$lanes" "$logs_for_node" || exit 1

start() {
  name=$1
  shift
  (
    "$@" > "$logs/$name.log" 2>&1
    echo $? > "$logs/$name.exit"
  ) &
}
lane() {
  cases=$logs_for_node/lane-$1.json
  code=0
  node "$runner" --cases "$cases" --ts "node rs/parity/with-mock.mjs ts" --rs "env SNOCOMMIT_PACKAGE_ROOT=$root node rs/parity/with-mock.mjs $bin" || code=1
  # On a terminal, into a log of its own: its "all N case(s)" line is not
  # the plain run's. All of it goes into the lane's log when it fails.
  case "$platform" in
    win32*) ;;
    *)
      tty_log=$logs/lane-$1.tty.log
      if ! node "$runner" --cases "$cases" --tty stdout,stderr --ts "node rs/parity/with-mock.mjs ts" --rs "env SNOCOMMIT_PACKAGE_ROOT=$root node rs/parity/with-mock.mjs $bin" > "$tty_log" 2>&1; then
        code=1
        echo "on a terminal (--tty stdout,stderr):"
        cat "$tty_log"
      fi
      sed -n 's/^all \([0-9]*\) case(s) behave the same$/on a terminal: \1/p' "$tty_log"
      ;;
  esac
  # (Not $name: start() still needs it when this returns.)
  for program in $(cat "$logs/lane-$1.commands"); do
    node "$runner" --cases "$cases" --filter "[$program]" --rs "node rs/parity/with-mock.mjs $bin" --handback "SNOCOMMIT_PACKAGE_ROOT=dist/cli-$program.js" || code=1
  done
  return $code
}
runs=""
n=0
while [ "$n" -lt "$lanes" ]; do
  start "lane$n" lane "$n"
  runs="$runs lane$n"
  n=$((n + 1))
done
for name in $others; do
  case $name in
    unit) start unit cargo test --release --locked --manifest-path rs/Cargo.toml ;;
  esac
  runs="$runs $name"
done
wait

# (`node --version` says nothing where node is a link to bun.)
node_is=$(node -e "process.stdout.write(process.versions.bun ? 'bun ' + process.versions.bun + ' as node' : process.version)")
echo "proof on $platform with node $node_is, bun $(bun --version 2> /dev/null || echo none), $lanes lane(s):$runs"
failed=0
same=0 native=0 back=0 on_tty=0
for name in $runs; do
  code=$(cat "$logs/$name.exit" 2> /dev/null || echo "no exit code")
  if [ "$code" != 0 ]; then
    echo "$name: FAILED ($code)"
    cat "$logs/$name.log"
    failed=1
    continue
  fi
  case $name in
    lane*)
      count=$(sed -n 's/^all \([0-9]*\) case(s) behave the same$/\1/p' "$logs/$name.log")
      same=$((same + count))
      for count in $(sed -n 's/^on a terminal: \([0-9]*\)$/\1/p' "$logs/$name.log"); do on_tty=$((on_tty + count)); done
      for count in $(sed -n 's/^\([0-9]*\) case(s) answered by the binary, .*/\1/p' "$logs/$name.log"); do native=$((native + count)); done
      for count in $(sed -n 's/^.* answered by the binary, \([0-9]*\) handed back .*/\1/p' "$logs/$name.log"); do back=$((back + count)); done
      ;;
    unit) grep '^test result' "$logs/$name.log" | sed "s/^/$name: /" ;;
  esac
done
if [ "$failed" = 0 ]; then
  echo "parity: all $same case(s) behave the same; $native answered by the binary, $back handed back to the JavaScript, as the cases say"
  case "$platform" in
    win32*) echo "on a terminal: not here (no pseudo-terminal on Windows)" ;;
    *) echo "on a terminal (--tty stdout,stderr): all $on_tty case(s) behave the same" ;;
  esac
  # The compare run of each lane names the cases skipped here, and why (the
  # hand-back runs skip them again, per command): the reasons, from the
  # compare runs only.
  tab=$(printf '\t')
  for name in $runs; do
    case $name in lane*) sed -n '1,/^all [0-9]* case(s) behave the same$/p' "$logs/$name.log" ;; esac
  done | sed -n "s/^ *\([0-9][0-9]*\)  \(.*\)\$/\1$tab\2/p" | sort -t "$tab" -k 2 | awk -F "$tab" '
    $2 != reason { if (reason != "") print total "\t" reason; reason = $2; total = 0 }
    { total += $1 }
    END { if (reason != "") print total "\t" reason }' | while IFS="$tab" read -r total reason; do
    echo "  skipped on $platform: $total  $reason"
  done
fi
case "$platform" in
  win32* | darwin*) ;;
  *) echo "unit: not here (the workflow's cargo test job runs it on Linux)" ;;
esac
rm -rf "$logs"
exit $failed
