"""Stable envelope for the legacy offline analyzer namespace."""

OUTPUT_NAMESPACE = "forgeos.legacy-ai-batch"
OUTPUT_VERSION = 1
AFDS_DIRECT_WRITE = False


def output_envelope() -> dict:
    """Metadata carried by every machine-readable public command output."""
    return {
        "namespace": OUTPUT_NAMESPACE,
        "version": OUTPUT_VERSION,
        "afds_direct_write": AFDS_DIRECT_WRITE,
    }


def format_output_contract() -> str:
    """Compact human-output declaration of the same contract."""
    return (f"Output contract: namespace={OUTPUT_NAMESPACE}; "
            f"version={OUTPUT_VERSION}; afds_direct_write=false")
