FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r requirements.txt

COPY bridge.py /app/bridge.py

ENV PYTHONUNBUFFERED=1
USER nobody

ENTRYPOINT ["python", "/app/bridge.py"]
