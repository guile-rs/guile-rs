(use-modules (rustup build toolchain)
	     (guix build utils)
	     (guix packages))

(packages->manifest
 (append (list (rustup "1.88.0"
		       #:components (list "cargo" "rustc" "rustfmt" "rust-std"
					  "rust-src" "clippy" "miri" "rust-analyzer"
					  "rust-mingw")))
	 (specifications->packages
	  '(
	    "coreutils"
	    "gcc-toolchain"
	    "binutils"
	    "pkg-config"
	    "clang"
	    "llvm"
	    "guile"
	    ))))
