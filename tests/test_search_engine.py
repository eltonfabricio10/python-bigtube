from unittest.mock import patch

from bigtube.core.search import SearchEngine


def test_search_engine_init_does_not_run_blocking_updater():
    with (
        patch("bigtube.core.search.ConfigManager.get_yt_dlp_path", return_value="/usr/bin/yt-dlp"),
        patch("bigtube.core.search.ConfigManager.get_env_with_bin_path", return_value={}),
        patch("bigtube.core.updater.Updater.ensure_exists") as ensure_exists,
    ):
        engine = SearchEngine()

    assert engine.binary_path == "/usr/bin/yt-dlp"
    ensure_exists.assert_not_called()
