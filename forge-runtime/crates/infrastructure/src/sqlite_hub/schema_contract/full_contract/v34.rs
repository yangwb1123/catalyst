// Values are derived from the complete v1..=v34 migration chain after review.
pub(super) const V34_IMPLICIT_INDEX_COUNT: usize = 124;
pub(super) const V34_CATALOG_IMPLICIT_INDEX_COUNT: usize = 115;
pub(super) const V34_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x79, 0x60, 0x3e, 0xa5, 0x75, 0x9a, 0x76, 0x18, 0x17, 0xb2, 0xbb, 0x74, 0x0e, 0x61, 0xf4, 0xe3,
    0x38, 0x64, 0x25, 0xc1, 0xd4, 0x5a, 0x5d, 0xac, 0x88, 0xd6, 0x30, 0x49, 0x4f, 0x21, 0xf5, 0x79,
];
pub(super) const V34_STRUCTURAL_CONTRACT: (usize, [u8; 32]) =
    (V34_IMPLICIT_INDEX_COUNT, V34_STRUCTURAL_CONTRACT_SHA256);

pub(super) const fn catalog_implicit_index_count(
    _version: usize,
    _signature_count: usize,
) -> usize {
    V34_CATALOG_IMPLICIT_INDEX_COUNT
}
