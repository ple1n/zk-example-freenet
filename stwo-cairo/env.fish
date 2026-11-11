set -l source_path (status filename)
set -gx STWO_CAIRO_DIR (cd (dirname $source_path); and pwd)
