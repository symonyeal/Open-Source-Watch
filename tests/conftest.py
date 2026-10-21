from pathlib import Path

import pytest

from engine import cycle, process

FIX = Path(__file__).parent / "fixtures"


@pytest.fixture(autouse=True)
def data(tmp_path, monkeypatch):
    """Each test gets an empty store and no translation network calls."""
    monkeypatch.setenv("OSW_DATA", str(tmp_path))
    monkeypatch.setattr(cycle, "_seen", None)
    monkeypatch.setattr(process, "_cache", {})
    return tmp_path
