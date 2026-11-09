# Supplemental frontend attribution and MIT text

This is a distributor-prepared supplement for the Candidate review, not an
upstream LICENSE file, a new license grant or distribution approval. The
packages below declare MIT or ISC but do not include a complete root notice in
the preserved published version. Their original metadata and README files must
be retained unchanged alongside this supplement.

Author attribution below is taken from package metadata. It is not a fabricated
copyright declaration: no copyright year or ownership assignment is inferred.
The standard MIT permission/warranty text is supplied explicitly as a supplement
to the original MIT declarations. Acceptance of this treatment remains part of
the release review described in
[LIBMPV-LICENSE-CLOSURE.md](LIBMPV-LICENSE-CLOSURE.md).

## is-reference 3.0.3

- Author as declared: Rich Harris.
- Declared license: MIT in package.json and README.
- Recorded repository revision: `8bb053129bfabe2f6a7d7ed050159d67ebe82829`.
- Preserve the original package metadata, README and source files. No complete
  LICENSE file was found in that version's distribution or recorded repository
  snapshot.

## locate-character 3.0.0

- Author as declared: Rich Harris.
- Declared license: MIT in package.json and README.
- Recorded repository revision: `4f08a59ec248121f7002abd02ee7b94e8eda06bc`.
- Preserve the original package metadata, README and source files. The manifest
  identifies GitLab; the preserved GitHub mirror is identified separately. No
  complete LICENSE file was found in the examined distribution or snapshot.

## @hugeicons/core-free-icons 4.2.3

- Author as declared: Hugeicons.
- Declared license: MIT in package.json.
- Preserve the original README, including its attribution and rights statement.
- The licensor explicitly identifies this free package as MIT licensed at
  <https://hugeicons.com/license-agreement>. The free icons are excluded from the
  agreement's Pro restrictions; this project does not apply those restrictions
  to the free package.
- A complete notice from version 4.3.5 is preserved as later-version evidence
  only. Its copyright date is not silently assigned to the installed 4.2.3
  package. The separate Svelte wrapper license is not substituted either.

## saxes 6.0.0

- Author as declared: Louis-Dominique Dubeau.
- Declared license: ISC in the npm registry metadata; the exact published npm
  tarball has no LICENSE file.
- Recorded source revision: `211fa0ebec9b628affc09219199639887174bfc3`.
- Preserve the npm package metadata, README and source files alongside the full
  upstream LICENSE text from the exact source revision. Its current ISC notice,
  historical `sax` notice and obsolete `String.fromCodePoint` notice are copied
  together; the historical texts are not represented as new grants for the
  current package.
- `saxes` is a transitive dependency of `jsdom`, which is only a development
  dependency, and its name does not occur in the built static UI assets. This
  supplement is therefore for the frontend source-package archive if that
  archive includes the npm package, not a claim that `saxes` ships in the
  application runtime.

The exact upstream LICENSE text follows:

```text
The ISC License

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

====

The following license is the one that governed sax, from which saxes
was forked. Isaac Schlueter is not *directly* involved with saxes so
don't go bugging him for saxes issues.

The ISC License

Copyright (c) Isaac Z. Schlueter and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

====

`String.fromCodePoint` by Mathias Bynens is no longer used, but it can
still be found in old commits. It was once used according to terms of
MIT License, as follows:

    Copyright Mathias Bynens <https://mathiasbynens.be/>

    Permission is hereby granted, free of charge, to any person obtaining
    a copy of this software and associated documentation files (the
    "Software"), to deal in the Software without restriction, including
    without limitation the rights to use, copy, modify, merge, publish,
    distribute, sublicense, and/or sell copies of the Software, and to
    permit persons to whom the Software is furnished to do so, subject to
    the following conditions:

    The above copyright notice and this permission notice shall be
    included in all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
    EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
    MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
    NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
    BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
    ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING FROM,
    OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
    IN THE SOFTWARE.
```

## Standard MIT permission and warranty text

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
