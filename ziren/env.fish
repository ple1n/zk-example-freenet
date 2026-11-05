set -gx ZKM_ENV (set -q ZKM_ENV; and echo $ZKM_ENV; or echo $HOME/.zkm-toolchain/env)
if not test -r "$ZKM_ENV"
    echo "Ziren toolchain environment not found at $ZKM_ENV (set ZKM_ENV to override)." >&2
    return 1
end

for entry in (bash -c 'source "$1" >/dev/null && env -0' _ "$ZKM_ENV" | string split0)
    set -l pair (string split -m 1 = -- $entry)
    set -gx $pair[1] $pair[2]
end
