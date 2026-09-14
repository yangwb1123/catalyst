// v30 structural constants are pinned to the baseline and change journal DDL.
pub(super) const V30_IMPLICIT_INDEX_COUNT: usize = 109;
pub(super) const V30_CATALOG_IMPLICIT_INDEX_COUNT: usize = 107;
pub(super) const V30_STRUCTURAL_CONTRACT_SHA256: [u8; 32] = [
    0xfd, 0xc6, 0x3f, 0xbc, 0x80, 0xe9, 0x7b, 0x20, 0x8a, 0xbd, 0x53, 0xe4, 0x7a, 0x88, 0x02, 0x91,
    0x1c, 0xa0, 0xe2, 0x83, 0x05, 0x0a, 0x95, 0xfb, 0x39, 0x31, 0x47, 0xa1, 0x9e, 0x40, 0xb4, 0xe8,
];
pub(super) const V30_STRUCTURAL_CONTRACT: (usize, [u8; 32]) =
    (V30_IMPLICIT_INDEX_COUNT, V30_STRUCTURAL_CONTRACT_SHA256);

pub(super) fn catalog_implicit_index_count(version: usize, signature_count: usize) -> usize {
    if version == 30 {
        V30_CATALOG_IMPLICIT_INDEX_COUNT
    } else {
        signature_count
    }
}
