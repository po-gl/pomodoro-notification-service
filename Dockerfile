FROM rust:1.74

WORKDIR /opt/pomodoro-notification-service
COPY . .

RUN cargo install --path .

ENV PORT=80
EXPOSE 80

CMD ["pomodoro-notification-service"]
