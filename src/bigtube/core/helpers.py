from .enums import DownloadStatus
from .locales import ResourceManager as Res, StringKey


def get_status_label(status_input) -> str:
    """
    Converts a DownloadStatus into a localized UI string.
    Example: DownloadStatus.COMPLETED -> "Concluído" (in PT)
    """

    # 1. Normalize input: Ensure we are comparing Enums
    # If the input comes from JSON history,
    # it might be a raw string like "completed"
    status_enum = status_input
    if isinstance(status_input, str):
        try:
            status_enum = DownloadStatus(status_input)
        except ValueError:
            # Fallback if the string in JSON doesn't match any Enum
            return Res.get(StringKey.STATUS_PENDING)

    # 2. Define Mapping: Enum -> Localization Key
    mapping = {
        DownloadStatus.PENDING: StringKey.STATUS_PENDING,
        DownloadStatus.DOWNLOADING: StringKey.STATUS_DOWNLOADING,
        DownloadStatus.COMPLETED: StringKey.STATUS_COMPLETED,
        DownloadStatus.ERROR: StringKey.STATUS_ERROR,
        DownloadStatus.CANCELLED: StringKey.STATUS_CANCELLED,
        DownloadStatus.INTERRUPTED: StringKey.STATUS_INTERRUPTED
    }

    # 3. Retrieve Key
    string_key = mapping.get(status_enum, StringKey.STATUS_PENDING)

    # 4. Translate
    return Res.get(string_key)


def is_youtube_url(url: str) -> bool:
    """Checks if the URL belongs to YouTube."""
    if not url:
        return False
    return "youtube.com" in url or "youtu.be" in url
