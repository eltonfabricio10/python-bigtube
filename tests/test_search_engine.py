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


def test_youtube_music_search_uses_music_search_url_and_filters_non_watch_entries():
    with (
        patch("bigtube.core.search.ConfigManager.get_yt_dlp_path", return_value="/usr/bin/yt-dlp"),
        patch("bigtube.core.search.ConfigManager.get_env_with_bin_path", return_value={}),
        patch("bigtube.core.search.ConfigManager.get", return_value=10),
    ):
        engine = SearchEngine()

    captured = {}

    def fake_run_cli(args, force_audio=False, query=None, source=None):
        captured["args"] = args
        captured["force_audio"] = force_audio
        captured["query"] = query
        captured["source"] = source
        return []

    with patch.object(engine, "_run_cli", side_effect=fake_run_cli):
        engine.search("shania twain", source="youtube_music")

    assert captured["force_audio"] is True
    assert captured["source"] == "youtube_music"
    assert "https://music.youtube.com/search?q=shania+twain" in captured["args"]

    assert engine._should_skip_entry(
        {"url": "https://music.youtube.com/browse/MPREb_example"}, "youtube_music"
    )
    assert not engine._should_skip_entry(
        {"url": "https://music.youtube.com/watch?v=abc123"}, "youtube_music"
    )
