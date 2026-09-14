// v31 structural constants are pinned after deriving them from the complete
// v1..=v31 migration sequence and reviewing the exact v31 table signature.
pub(super) const V31_IMPLICIT_INDEX_COUNT: usize = 110;
pub(super) const V31_CATALOG_IMPLICIT_INDEX_COUNT: usize = 107;
pub(super) const V31_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0x56, 0x0c, 0xe0, 0xaa, 0x60, 0xba, 0x96, 0x83, 0x3f, 0xf0, 0xe7, 0x27, 0x2e, 0xe4, 0xfc, 0x52,
    0x9c, 0x00, 0xee, 0x17, 0xc0, 0x01, 0xc9, 0x77, 0x75, 0xde, 0x5a, 0x1e, 0x46, 0xbe, 0xfa, 0x88,
];
pub(super) const V31_STRUCTURAL_CONTRACT: (usize, [u8; 32]) =
    (V31_IMPLICIT_INDEX_COUNT, V31_STRUCTURAL_CONTRACT_SHA256);

pub(super) const fn catalog_implicit_index_count(
    _version: usize,
    _signature_count: usize,
) -> usize {
    V31_CATALOG_IMPLICIT_INDEX_COUNT
}
