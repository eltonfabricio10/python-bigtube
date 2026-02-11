
import pytest
from bigtube.core.validators import sanitize_filename, sanitize_url, is_valid_url

class TestValidators:
    @pytest.mark.parametrize("input_name,expected", [
        ("My Video Title", "My Video Title"), # Implementation allows spaces
        ("Video with / slashes", "Video with  slashes"),
        ("Complex: Name!", "Complex Name"),
        ("..", "untitled"),  # sanitize_filename handles dots by stripping them, if empty returns untitled
        ("", "untitled"),
        ("unicode_😊_test", "unicode_test"), # Regex [^\w\s\-_().[\]] might remove emoji depending on python version and unicode flag
    ])
    def test_sanitize_filename(self, input_name, expected):
        # NOTE: The implementation uses [\w] which matches alphanumerics.
        # It removes colons, slashes, etc.
        assert sanitize_filename(input_name) == expected

    def test_sanitize_filename_length(self):
        long_name = "a" * 255
        sanitized = sanitize_filename(long_name, max_length=50)
        assert len(sanitized) <= 50

    @pytest.mark.parametrize("url,expected", [
        ("www.youtube.com/watch?v=123", "https://www.youtube.com/watch?v=123"),
        ("  https://google.com  ", "https://google.com"),
    ])
    def test_sanitize_url(self, url, expected):
        assert sanitize_url(url) == expected

    @pytest.mark.parametrize("url,is_valid", [
        ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", True),
        ("not a url", False),
        ("ftp://example.com", False), # Not in allowed patterns?
    ])
    def test_is_valid_url(self, url, is_valid):
        assert is_valid_url(url) == is_valid
