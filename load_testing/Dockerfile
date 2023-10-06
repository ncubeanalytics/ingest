FROM python:3.11.2-bullseye

WORKDIR /app

COPY data.jsonl .
COPY requirements.txt .
RUN pip install -r requirements.txt
COPY load.py .

ENTRYPOINT ["python", "load.py"]
