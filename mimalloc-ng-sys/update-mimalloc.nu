#!/usr/bin/env nu
# Replaces the vendored mimalloc sources with the release tagged `tag`, e.g. v3.6.0.
def main [tag: string] {
   let dest = $env.FILE_PWD | path join mimalloc
   let tmp = mktemp --directory

   http get $"https://github.com/microsoft/mimalloc/archive/refs/tags/($tag).tar.gz"
   | save $"($tmp)/mimalloc.tar.gz"
   ^tar -xzf $"($tmp)/mimalloc.tar.gz" -C $tmp
   let upstream = ls $tmp | where type == dir | get name | first

   rm --recursive --force $dest
   mkdir $dest
   cp --recursive $"($upstream)/include" $"($upstream)/src" $"($upstream)/LICENSE" $dest
   glob $"($dest)/**/readme.md" | each {|file| rm $file } | ignore
   rm --recursive $tmp
}
