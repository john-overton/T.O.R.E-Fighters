# Third-party notices

## PKWare DCL decoding

The Rust raw-literal DCL decoder in `crates/tore-formats/src/dcl.rs` uses the
format tables, canonical decoding method, and public test vector documented by
Mark Adler's [blast](https://github.com/madler/zlib/tree/master/contrib/blast),
also consulted through the local USNF-ATF reference decoder. This is an altered,
bounded Rust implementation, not the original blast distribution. It supports
raw literals only, limits output, and returns Rust I/O errors.

The upstream notice from `blast.h` follows:

```text
Copyright (C) 2003, 2012, 2013 Mark Adler

This software is provided 'as-is', without any express or implied
warranty.  In no event will the author be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.

Mark Adler    madler@alumni.caltech.edu
```

## Local game data and reference specifications

Game art, fonts, sounds, and other retail resources remain user-supplied. They
are not licensed or distributed by this repository. Format observations were
checked against the local `john-overton/USNF-ATF` checkout and Fighters Anthology
media; see `docs/formats/menu.md` for provenance and validation scope.

Rust dependency licenses remain those published by their respective authors.
`Cargo.lock` records the versions used.
