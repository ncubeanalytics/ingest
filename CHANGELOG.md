# Changelog

## 0.9.0 - 2024-07-08

### Changed

* Configuration changes

## 0.8.0 - 2023-12-27

### Changed

* Pass headers to python as a dict-like object instead of list of tuples

## 0.7.0 - 2023-12-20

### Added

* Support arbitrary trailing path in url after schema id

## 0.6.0 - 2023-12-20

### Added

* Support independent librdkafka clients per schema
* Install python-dev in docker image and default PYTHONPATH directory `/opt/ingest/python`

### Fixed

* Read Content-Type header correctly in presence of parameters
