from bigtube.controllers.search_controller import search_urls_parallel


class FakeSearchEngine:
    def search(self, url, source="youtube"):
        assert source == "url"
        return [{"url": url}]


def test_search_urls_worker_collects_multiple_url_results():
    results, errors = search_urls_parallel(
        FakeSearchEngine(), ["https://one.test", "https://two.test"]
    )

    assert errors == []
    assert {item["url"] for item in results} == {"https://one.test", "https://two.test"}
