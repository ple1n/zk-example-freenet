set -gx DIR (cd (dirname (status filename)); and pwd)
if not set -q SP1_MERKLE_DEPTH
    set -gx SP1_MERKLE_DEPTH 16
else
    set -gx SP1_MERKLE_DEPTH $SP1_MERKLE_DEPTH
end
set -l target_dir (cargo metadata --no-deps --format-version 1 --manifest-path "$DIR/Cargo.toml" | jq -er .target_directory)
or return 1
set -gx TARGET_DIR $target_dir
