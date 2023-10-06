from __future__ import annotations

import argparse
import asyncio
import configparser
import json
import logging
import logging.config
import random
import os
from urllib.parse import urlparse
from time import monotonic
from typing import Iterator, TypedDict


logger = logging.getLogger("load_test")


class Measurement(TypedDict):
    items: int
    total_time_taken_seconds: float


class DataGen:
    def __init__(self, limit: int):
        with open(os.path.join(os.path.dirname(__file__), "data.jsonl"), "rb") as f:
            self.data = f.read().splitlines()
        self.limit = limit

    def data_iter(self) -> Iterator[bytes]:
        limit = self.limit
        data = self.data
        cnt = 0
        while limit is None or cnt < limit:
            yield random.choice(data)
            cnt += 1


async def eventhubs(
    data: Iterator[bytes], conn_str: str, event_hub: str
) -> Measurement:
    # bufferred: {"items": 2858, "total_time_taken_seconds": 30.428403499987326}
    from azure.eventhub import EventData
    from azure.eventhub.aio import EventHubProducerClient

    start = monotonic()
    total_items_sent = 0
    items_acked = 0

    done_queue = asyncio.Queue()
    done = object()

    async def on_success(events, partition_id):
        nonlocal items_acked
        items_acked += len(events)

        if total_items_sent == items_acked:
            await done_queue.put(done)

    async def on_error(events, partition_id, error):
        raise Exception(error)

    producer = EventHubProducerClient.from_connection_string(
        conn_str=conn_str,
        eventhub_name=event_hub,
        buffered_mode=True,
        on_success=on_success,
        on_error=on_error,
    )
    async with producer:
        event_data_batch = await producer.create_batch()
        i = -1
        for i, d in enumerate(data):
            try:
                event_data_batch.add(EventData(d))
            except ValueError:
                await producer.send_batch(event_data_batch)
                event_data_batch = await producer.create_batch()
                event_data_batch.add(EventData(d))
        if len(event_data_batch):
            await producer.send_batch(event_data_batch)
        total_items_sent = i + 1

    await done_queue.get()
    total_time_taken = monotonic() - start
    return dict(
        items=total_items_sent,
        total_time_taken_seconds=total_time_taken,
    )


def librdkafka(data: Iterator[bytes], producer_config: dict, topic: str) -> Measurement:
    # {"items": 3800, "total_time_taken_seconds": 30.01175879998482}
    import queue
    import confluent_kafka

    def on_kafka_error(err: confluent_kafka.KafkaError):
        if not err:
            return
        if err.retriable():
            logger.debug("Retriable Kafka error %s", err)
        if err.fatal():
            raise Exception(f"Kafka failed with error {err}")
        logger.debug("Non-fatal Kafka error %s", err)

    start = monotonic()
    items_acked = 0

    def on_delivery(err, msg):
        nonlocal items_acked
        if err:
            on_kafka_error(err)
        elif msg:
            items_acked += 1

    producer = confluent_kafka.Producer(
        dict(
            producer_config,
            # on_delivery=lambda err, msg: on_kafka_error(err, "producer delivery"),
            on_delivery=on_delivery,
            error_cb=lambda err: on_kafka_error(err),
        )
    )

    i = 0
    for i, d in enumerate(data):
        try:
            producer.produce(
                topic=topic,
                value=d,
                # on_delivery=on_delivery,
            )
            # producer.poll(0)
        except BufferError as ex:
            logger.warning("%s", ex)
            producer.poll(120)
            producer.produce(
                topic=topic,
                value=d,
                # on_delivery=on_delivery,
            )
    total_items_sent = i + 1

    while True:
        producer.poll(1)
        if total_items_sent == items_acked:
            total_time_taken = monotonic() - start
            return dict(
                items=total_items_sent,
                total_time_taken_seconds=total_time_taken,
            )


def ingest(data: Iterator[bytes], url: str) -> Measurement:
    # {"items": 105, "total_time_taken_seconds": 30.548640299995895
    import requests
    import requests.adapters

    items = 0
    start = monotonic()

    s = requests.Session()
    adapter = requests.adapters.HTTPAdapter(
        pool_connections=1,  # number of different hosts
        pool_maxsize=1,  # number of maximum concurrent requests
    )
    s.mount("http://", adapter)
    s.mount("https://", adapter)
    s.headers["Content-Type"] = "application/json"

    for d in data:
        r = s.post(url, data=d)
        r.raise_for_status()

        items += 1
        # if items % 5 == 0:
        #     total_time_taken = monotonic() - start
        #     logger.debug(
        #         "%",
        #         json.dumps(
        #             dict(
        #                 items=items,
        #                 total_time_taken_seconds=total_time_taken,
        #             )
        #         ),
        #     )

    total_time_taken = monotonic() - start
    return dict(
        items=items,
        total_time_taken_seconds=total_time_taken,
    )


async def ingest_asyncio(
    data: Iterator[bytes], url: str, parallelism: int
) -> Measurement:
    # 100: {"items": 4100, "total_time_taken_seconds": 30.1827992000035}
    import asyncio
    import aiohttp

    results_queue = asyncio.Queue()
    done = object()

    async def worker():
        cnt = 0
        async with aiohttp.ClientSession() as s:
            for d in data:
                start = monotonic()
                async with s.post(url, data=d) as r:
                    r.raise_for_status()
                    elapsed = monotonic() - start
                    await results_queue.put(elapsed)
                cnt += 1
        await results_queue.put(done)

    async def measurer():
        items = 0
        start = monotonic()
        dones = 0
        while True:
            maybe_done = await results_queue.get()
            if maybe_done is done:
                dones += 1
                if dones >= parallelism:
                    break
            else:
                items += 1

        total_time_taken = monotonic() - start
        return dict(
            items=items,
            total_time_taken_seconds=total_time_taken,
        )

    tasks = []
    tasks.append(asyncio.create_task(measurer()))
    for i in range(parallelism):
        tasks.append(asyncio.create_task(worker()))
    results = await asyncio.gather(*tasks)
    return results[0]


def ingest_jsonlines(
    data: Iterator[bytes], url: str, chunk_size_bytes: int
) -> Measurement:
    # 55: {"items": 2200, "total_time_taken_seconds": 30.721066700003576}
    # 200: {"items": 3400, "total_time_taken_seconds": 30.537815500007127}
    # 300: {"items": 3600, "total_time_taken_seconds": 29.54513670000597}
    import requests
    from io import BytesIO
    import requests.adapters

    s = requests.Session()
    adapter = requests.adapters.HTTPAdapter(
        pool_connections=1,  # number of different hosts
        pool_maxsize=1,  # number of maximum concurrent requests
    )
    s.mount("http://", adapter)
    s.mount("https://", adapter)

    items = 0
    start = monotonic()

    cur_chunk_size = 0
    b = BytesIO()

    def flush():
        nonlocal b, cur_chunk_size
        b.seek(0)
        r = s.post(
            url,
            headers={"Content-Type": "application/jsonlines"},
            data=b,
            # timeout=2,
        )
        r.raise_for_status()
        cur_chunk_size = 0
        b = BytesIO()

    for d in data:
        if cur_chunk_size + len(d) + 1 > chunk_size_bytes:
            flush()
        b.write(d)
        b.write(b"\n")
        cur_chunk_size += len(d) + 1
        items += 1

    if cur_chunk_size:
        flush()

    total_time_taken = monotonic() - start
    return dict(
        items=items,
        total_time_taken_seconds=total_time_taken,
    )


def wait_for_service(host: str, port: int, timeout: int = 60) -> None:
    import socket

    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(timeout)
    try:
        s.connect((host, port))
    finally:
        s.close()


def setup_logging(
    filename: str = None,
    debug_http=False,
    debug_azure_identity=False,
    level="DEBUG",
) -> None:
    config = {
        "version": 1,
        "disable_existing_loggers": True,
        "formatters": {
            "standard": {
                "format": "[%(asctime)s] %(levelname)s [%(name)s:%(lineno)s] [pid:%(process)d:%(threadName)s] %(message)s",
                "datefmt": "%d/%b/%Y %H:%M:%S",
            }
        },
        "handlers": {
            "stdout": {
                "level": level,
                "class": "logging.StreamHandler",
                "formatter": "standard",
                "stream": "ext://sys.stdout",
            }
        },
        "loggers": {
            "urllib3": {"handlers": [], "level": "DEBUG" if debug_http else "INFO"},
            "azure": {
                "handlers": [],
                "level": "DEBUG" if debug_azure_identity else "WARN",
            },
            "msal": {
                "handlers": [],
                "level": "DEBUG" if debug_azure_identity else "WARN",
            },
            "load_test": {},
        },
        "root": {"handlers": ["stdout"], "level": level},
    }
    if filename:
        config["handlers"]["file"] = {
            "level": "DEBUG",
            "class": "logging.FileHandler",
            "formatter": "standard",
            "filename": filename,
        }
        config["root"]["handlers"].append("file")
    logging.config.dictConfig(config)


def main():
    parser = argparse.ArgumentParser(description="Run ingest load tests")

    parser.add_argument(
        "--impl",
        required=True,
        nargs="+",
        choices=[
            "ingest-plain",
            "ingest-batch",
            "ingest-asyncio",
            "librdkafka",
            "eventhubs-sdk",
        ],
    )

    parser.add_argument("--ingest-item-count", type=int, default=1000)
    parser.add_argument("--ingest-service-url", default="http://127.0.0.1:8088")
    parser.add_argument("--ingest-service-schema-id", default="1")
    parser.add_argument(
        "--ingest-batch-chunk-size-bytes", type=int, default=5 * 1024 * 1024
    )
    parser.add_argument("--ingest-asyncio-concurrency", type=int, default=100)
    parser.add_argument("--librdkafka-producer-config", metavar="PATH")
    parser.add_argument("--eventhubs-conn-str", metavar="PATH")
    parser.add_argument("--eventhub-name")

    args = parser.parse_args()

    setup_logging()

    needs_ingest = any(
        i in args.impl for i in ["ingest-plain", "ingest-batch", "ingest-asyncio"]
    )

    if needs_ingest:
        if args.ingest_service_url is None:
            raise parser.error(
                "--ingest-service-url is mandatory for --impl ingest-plain ingest-batch ingest-asyncio"
            )
        if args.ingest_service_schema_id is None:
            raise parser.error(
                "--ingest-service-schema-id is mandatory for --impl ingest-plain ingest-batch ingest-asyncio"
            )
    if any(i in args.impl for i in ["librdkafka", "eventhubs-sdk"]):
        if args.eventhub_name is None:
            raise parser.error(
                "--eventhub-name is mandatory for --impl librdkafka eventhubs-sdk"
            )
    if "eventhubs-sdk" in args.impl and args.eventhubs_conn_str is None:
        raise parser.error("--eventhubs-conn-str is mandatory for --impl eventhubs-sdk")
    if "librdkafka" in args.impl and args.librdkafka_producer_config is None:
        raise parser.error(
            "--librdkafka-producer-config is mandatory for --impl librdkafka"
        )

    data_gen = DataGen(limit=args.ingest_item_count)

    if needs_ingest:
        parsed_ingest_url = urlparse(args.ingest_service_url)
        wait_for_service(parsed_ingest_url.hostname, parsed_ingest_url.port)
        url = args.ingest_service_url + "/" + args.ingest_service_schema_id
    results = []
    for impl in args.impl:
        if impl == "ingest-plain":
            logger.debug("Starting %s", impl)
            ingest_result = ingest(data_gen.data_iter(), url)
            logger.info("%s results: %s", impl, json.dumps(ingest_result))
            results.append((impl, ingest_result))
        elif impl == "ingest-batch":
            logger.debug("Starting %s", impl)
            ingest_jsonlines_result = ingest_jsonlines(
                data_gen.data_iter(), url, args.ingest_batch_chunk_size_bytes
            )
            logger.info("%s results: %s", impl, json.dumps(ingest_jsonlines_result))
            results.append((impl, ingest_jsonlines_result))
        elif impl == "ingest-asyncio":
            logger.debug("Starting %s", impl)
            ingest_asyncio_result = asyncio.run(
                ingest_asyncio(
                    data_gen.data_iter(), url, args.ingest_asyncio_concurrency
                )
            )
            logger.info("%s results: %s", impl, json.dumps(ingest_asyncio_result))
            results.append((impl, ingest_asyncio_result))
        elif impl == "librdkafka":
            config = configparser.ConfigParser()
            with open(args.librdkafka_producer_config) as f:
                config.read_file(f)
            producer_config = dict(
                config.items("producer"),
            )
            if args.eventhubs_conn_str:
                with open(args.eventhubs_conn_str) as f:
                    sasl_password = f.read()
                producer_config["sasl.password"] = sasl_password
            logger.debug("Starting %s", impl)
            librdkafka_result = librdkafka(
                data_gen.data_iter(), producer_config, args.eventhub_name
            )
            logger.info("%s results: %s", impl, json.dumps(librdkafka_result))
            results.append((impl, librdkafka_result))
        elif impl == "eventhubs-sdk":
            with open(args.eventhubs_conn_str) as f:
                conn_str = f.read()
            logger.debug("Starting %s", impl)
            eventhubs_sdk_result = asyncio.run(
                eventhubs(data_gen.data_iter(), conn_str, args.eventhub_name)
            )
            logger.info("%s results: %s", impl, json.dumps(eventhubs_sdk_result))
            results.append((impl, eventhubs_sdk_result))
        else:
            raise Exception(f"Unrecognized impl f{impl}")

    logger.info("Done. Results:\n%s", results)


if __name__ == "__main__":
    main()
