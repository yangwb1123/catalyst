// v33 structural constants are pinned after deriving them from the complete
// v1..=v33 migration sequence and reviewing immutable consent history.
pub(super) const V33_IMPLICIT_INDEX_COUNT: usize = 118;
pub(super) const V33_CATALOG_IMPLICIT_INDEX_COUNT: usize = 111;
pub(super) const V33_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xef, 0x67, 0xe7, 0xa1, 0xd4, 0x6b, 0xa7, 0xb4, 0x3b, 0x49, 0x04, 0x96, 0xd3, 0x6f, 0x7b, 0xd1,
    0xe7, 0x56, 0x32, 0x9b, 0x42, 0x29, 0xf3, 0x77, 0x77, 0x1a, 0xe3, 0xc1, 0xf6, 0x5a, 0x1b, 0x98,
];
pub(super) const V33_STRUCTURAL_CONTRACT: (usize, [u8; 32]) =
    (V33_IMPLICIT_INDEX_COUNT, V33_STRUCTURAL_CONTRACT_SHA256);

pub(super) const fn catalog_implicit_index_count(
    _version: usize,
    _signature_count: usize,
) -> usize {
    V33_CATALOG_IMPLICIT_INDEX_COUNT
}
