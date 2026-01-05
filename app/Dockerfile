FROM mcr.microsoft.com/playwright/python:v1.57.0-jammy

WORKDIR /app

COPY requirements.txt ./
RUN pip install playwright
RUN pip install --no-cache-dir -r requirements.txt

CMD ["python", "e-ink.py"]
