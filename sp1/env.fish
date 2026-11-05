set -gx DIR (cd (dirname (status filename)); and pwd)
if not set -q SP1_MERKLE_DEPTH
    set -gx SP1_MERKLE_DEPTH 16
else
    set -gx SP1_MERKLE_DEPTH $SP1_MERKLE_DEPTH
end
