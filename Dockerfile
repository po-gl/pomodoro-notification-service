FROM rust:1.74

WORKDIR /opt/pomodoro-notification-service
COPY . .

EXPOSE 80

RUN cargo install --path .

CMD ["pomodoro-notification-service"]
