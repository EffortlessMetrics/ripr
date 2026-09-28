from src.callbacks import recording_callback


def test_recording_callback_starts_idle():
    callback = recording_callback()
    callback.assert_not_called()
