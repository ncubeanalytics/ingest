# Changelog

## 0.6.0 - 2023-12-20

### Added

* Support independent librdkafka clients per schema
* Install python-dev in docker image and default PYTHONPATH directory `/opt/ingest/python`

### Fixed

* Read Content-Type header correctly in presence of parameters