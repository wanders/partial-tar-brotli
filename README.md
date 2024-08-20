PARTIAL TAR BROTLI
==================

A small tool that when given a list and a size budget creates a brotli
compressed tar archive with as many files fits in the size budget.


**Example**:
```
$ partial-tar-brotli --max-size=16777216 --output=debug-dumps.tar.br debug-dumps/*.json
debug-dumps/slowness-1724183470.json does not fit. Archive would be 16778136 bytes.
Done! 8 out of 13 files added (5 skipped)


$ brotli -d -c debug-dumps.tar.br | tar t
partial-tar-brotli-manifest.json
debug-dumps/slowness-1724087161.json
debug-dumps/slowness-1724088275.json
debug-dumps/slowness-1724090861.json
debug-dumps/slowness-1724092523.json
debug-dumps/slowness-1724092780.json
debug-dumps/slowness-1724093795.json
debug-dumps/slowness-1724094847.json
debug-dumps/slowness-1724095657.json
```

License
-------

[Apache License, Version 2.0](COPYING)
