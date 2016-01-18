wemo.rs
=======

**wemo.rs** is a Rust library for interacting with the Belkin WeMo line
of home automation products.

WeMo devices are known to change ports frequently and without reason.
One of the major goals of *wemo.rs* is to be tolerant of this behavior
and allow quick and efficient operation of WeMo devices with reasonable
recovery and failure modes.

Makes use of the [mio](https://github.com/carllerche/mio) networking library
for nonblocking IO and timeouts.

TODO
----

- Finish up the library for a `0.1.0` release. (Don't release the code as
  horrendous as it is now.)

  - Eliminate dependencies: `hyper`, `time`, `toml`.

  - Testing and CI builds.

  - Documentation.

- Support for querying device state (eg. Insight power consumption).

- Separate project: network microservice incorporating *wemo.rs* for
  control and coordination of all WeMo devices on the local network
  (essentially a proxyaggregation protocol).

License
-------

**BSD 4-clause**

Copyright (c) 2015-2016, Brandon Thomas. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. All advertising materials mentioning features or use of this software must
   display the following acknowledgement:

   This product includes software developed by Brandon Thomas (bt@brand.io,
   echelon@gmail.com).

4. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY COPYRIGHT HOLDER "AS IS" AND ANY EXPRESS OR
IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO
EVENT SHALL COPYRIGHT HOLDER BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER
IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.

See Also
--------
- [ouimeaux](https://github.com/iancmcc/ouimeaux), a Python WeMo library

