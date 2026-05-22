from bigtube.core.converter_history import ConverterHistoryManager


def test_converter_history_uses_cache_and_debounced_save(monkeypatch, tmp_path):
    history_file = tmp_path / "converter_history.json"
    load_calls = []
    scheduled = []

    monkeypatch.setattr(ConverterHistoryManager, "_FILE_PATH", str(history_file))
    monkeypatch.setattr(ConverterHistoryManager, "_cache", None)
    monkeypatch.setattr(ConverterHistoryManager, "_pending_save", False)
    monkeypatch.setattr(
        "bigtube.core.converter_history.load_json",
        lambda path, default: load_calls.append(path) or [],
    )
    monkeypatch.setattr(
        "bigtube.core.converter_history.GLib.timeout_add",
        lambda timeout, callback: scheduled.append((timeout, callback)) or 1,
    )

    ConverterHistoryManager.add_entry("/tmp/in.mp4", "/tmp/out.mkv", "mkv")
    ConverterHistoryManager.add_entry("/tmp/in2.mp4", "/tmp/out2.mp3", "mp3")

    assert len(load_calls) == 1
    assert len(scheduled) == 1
    assert len(ConverterHistoryManager.load()) == 2
