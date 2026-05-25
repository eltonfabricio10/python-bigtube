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


def test_youtube_music_accepts_ids_and_normalizes_playable_urls():
    with (
        patch("bigtube.core.search.ConfigManager.get_yt_dlp_path", return_value="/usr/bin/yt-dlp"),
        patch("bigtube.core.search.ConfigManager.get_env_with_bin_path", return_value={}),
        patch("bigtube.core.search.ConfigManager.get", return_value=10),
    ):
        engine = SearchEngine()

    entry = {"id": "abc123DEF_-", "url": "abc123DEF_-", "title": "Song"}

    assert not engine._should_skip_entry(entry, "youtube_music")

    parsed = engine._parse_entry(entry, force_audio=True)

    assert parsed["url"] == "https://music.youtube.com/watch?v=abc123DEF_-"
    assert parsed["is_video"] is False


def test_youtube_music_uses_artist_and_thumbnail_fallback_from_video_id():
    with (
        patch("bigtube.core.search.ConfigManager.get_yt_dlp_path", return_value="/usr/bin/yt-dlp"),
        patch("bigtube.core.search.ConfigManager.get_env_with_bin_path", return_value={}),
        patch("bigtube.core.search.ConfigManager.get", return_value=10),
    ):
        engine = SearchEngine()

    parsed = engine._parse_entry(
        {
            "id": "abc123DEF_-",
            "url": "abc123DEF_-",
            "title": "Song",
            "channel": "YouTube Music",
            "artists": [{"name": "Artist One"}, {"name": "Artist Two"}],
        },
        force_audio=True,
    )

    assert parsed["thumbnail"] == "https://i.ytimg.com/vi/abc123DEF_-/hqdefault.jpg"
    assert parsed["uploader"] == "Artist One, Artist Two"


def test_parse_entry_selects_largest_thumbnail_candidate():
    with (
        patch("bigtube.core.search.ConfigManager.get_yt_dlp_path", return_value="/usr/bin/yt-dlp"),
        patch("bigtube.core.search.ConfigManager.get_env_with_bin_path", return_value={}),
        patch("bigtube.core.search.ConfigManager.get", return_value=10),
    ):
        engine = SearchEngine()

    parsed = engine._parse_entry(
        {
            "id": "abc123DEF_-",
            "url": "https://music.youtube.com/watch?v=abc123DEF_-",
            "title": "Song",
            "thumbnails": [
                {"url": "https://example.com/small.jpg", "width": 60, "height": 60},
                {"url": "https://example.com/large.jpg", "width": 544, "height": 544},
            ],
            "artist": "Artist",
        },
        force_audio=True,
    )

    assert parsed["thumbnail"] == "https://example.com/large.jpg"
    assert parsed["uploader"] == "Artist"
