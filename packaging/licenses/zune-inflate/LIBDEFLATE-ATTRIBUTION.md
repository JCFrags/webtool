# libdeflate-derived portions

`zune-inflate` 0.2.54 states that its implementation is heavily based on Eric
Biggers' libdeflate. Inspection of the checksum-pinned crate identifies the
following affected files:

- `src/constants.rs`: decode-table constants, entry layouts, flags, and static
  decode-result tables.
- `src/utils.rs`: `make_decode_table_entry` and its explanatory text.
- `src/decoder.rs`: the Huffman decode-table construction and the related
  DEFLATE decoding structure.
- `src/crc/crc_tables.rs`: CRC-32 lookup tables. Its header expressly says the
  tables were obtained from Eric Biggers' libdeflate.
- `src/crc.rs`: use of the libdeflate-derived CRC-32 lookup tables.

Upstream project: <https://github.com/ebiggers/libdeflate>

Pinned upstream comparison sources retrieved on 2026-09-10 from libdeflate
revision `92e6a0db9fa848d742f9eb286c92afc60f2c3dda`:

- `lib/deflate_decompress.c`, SHA-256
  `f4299b3a688b5768c3a8790339e47e02a6e6247d3675b0fafe412a1fc9bd995f`
- `lib/crc32_tables.h`, SHA-256
  `2aa0c8590795ab7d2722bdcd1be816d4808b6e7d301b52bd5ed4c5c1c72b996e`
- `lib/crc32.c`, SHA-256
  `91d0155e5e879527f9a5dbc53f01ea2400fbb80a75b9dda71d867cdd53d18cca`

These observed files establish the matched lineage and scope. They are not the
unknown libdeflate revision on which the original Rust adaptation was based.
`zune-inflate` does not identify that revision.

libdeflate is distributed under the MIT license. The accompanying
`LIBDEFLATE-COPYING` is pinned to libdeflate revision
`275aa5141db6eda3587214e0f1d3a134768f557d` and preserves the Copyright 2016
Eric Biggers notice expressly implicated by zune-inflate's source. This is an
applicable notice snapshot, not a claim that the Rust port used that revision.
MIT is one of the alternatives declared by `zune-inflate`.
