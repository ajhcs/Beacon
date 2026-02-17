#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <source-dir> <destination-dir> <manifest-path>" >&2
  exit 1
fi

source_dir="$1"
destination_dir="$2"
manifest_path="$3"

if [[ ! -d "$source_dir" ]]; then
  echo "Source directory does not exist: $source_dir" >&2
  exit 1
fi

mkdir -p "$destination_dir"
manifest_dir="$(dirname "$manifest_path")"
mkdir -p "$manifest_dir"

source_dir="$(cd "$source_dir" && pwd)"
destination_dir="$(cd "$destination_dir" && pwd)"
manifest_path="$(cd "$manifest_dir" && pwd)/$(basename "$manifest_path")"

if command -v sha256sum >/dev/null 2>&1; then
  hash_file() {
    sha256sum "$1" | awk '{print $1}'
  }
elif command -v shasum >/dev/null 2>&1; then
  hash_file() {
    shasum -a 256 "$1" | awk '{print $1}'
  }
elif command -v openssl >/dev/null 2>&1; then
  hash_file() {
    openssl dgst -sha256 "$1" | awk '{print $2}'
  }
else
  echo "No SHA256 tool available (sha256sum, shasum, or openssl required)." >&2
  exit 1
fi

mapfile -t source_files < <(find "$source_dir" -type f | sort)
if [[ "${#source_files[@]}" -eq 0 ]]; then
  echo "No files found to install from source directory: $source_dir" >&2
  exit 1
fi

{
  echo "# fresnel-fir installer manifest"
  echo "source=$source_dir"
  echo "destination=$destination_dir"
  echo "generated_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  echo "path|bytes|sha256"
} > "$manifest_path"

for source_file in "${source_files[@]}"; do
  relative_path="${source_file#"$source_dir"/}"
  destination_path="$destination_dir/$relative_path"
  mkdir -p "$(dirname "$destination_path")"
  cp "$source_file" "$destination_path"

  size_bytes="$(wc -c < "$destination_path" | tr -d '[:space:]')"
  file_hash="$(hash_file "$destination_path")"
  printf '%s|%s|%s\n' "$relative_path" "$size_bytes" "$file_hash" >> "$manifest_path"
done

echo "Installed ${#source_files[@]} files to $destination_dir"
echo "Manifest written to $manifest_path"
