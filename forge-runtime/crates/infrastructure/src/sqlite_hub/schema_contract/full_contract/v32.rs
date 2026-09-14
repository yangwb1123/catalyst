// v32 structural constants are pinned after deriving them from the complete
// v1..=v32 migration sequence and reviewing the owner-local cursor schema.
pub(super) const V32_IMPLICIT_INDEX_COUNT: usize = 113;
pub(super) const V32_CATALOG_IMPLICIT_INDEX_COUNT: usize = 108;
pub(super) const V32_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x28, 0x9f, 0xed, 0xb3, 0x21, 0x01, 0x48, 0x72, 0xe9, 0x72, 0x1d, 0xd5, 0x6e, 0xf1, 0x84, 0x99,
    0xbd, 0x92, 0xf0, 0x8b, 0x94, 0xbe, 0x20, 0x77, 0x3d, 0x99, 0x00, 0x63, 0x62, 0xba, 0x85, 0xe3,
];
pub(super) const V32_STRUCTURAL_CONTRACT: (usize, [u8; 32]) =
    (V32_IMPLICIT_INDEX_COUNT, V32_STRUCTURAL_CONTRACT_SHA256);

pub(super) const fn catalog_implicit_index_count(
    _version: usize,
    _signature_count: usize,
) -> usize {
    V32_CATALOG_IMPLICIT_INDEX_COUNT
}
