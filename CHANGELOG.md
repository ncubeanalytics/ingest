# Changelog

## 0.12.0 - 2025-01-14

### Changed

* Changed configuration to support arbitrary librdkafka properties read from a file, not just 
  sasl password

## 0.11.0 - 2025-01-13

### Added

* Add support for all librdkafka features and corresponding debian packages

## 0.10.0 - 2024-10-06

### Added

* Emit `ingest.schema.id` metric attribute

### Changed

* Endpoints are now under the `/ingest` path, not the root

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
