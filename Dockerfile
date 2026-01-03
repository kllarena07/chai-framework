FROM alpine:latest

# replace 'app' for the name of your program
ARG APP_NAME=app

# copy the statically linked binary (you must build this statically on your host)
COPY ${APP_NAME} /usr/local/bin/${APP_NAME}

# Optional: copy any static assets if needed (uncomment and modify as required)
# COPY assets/ /app/assets/

EXPOSE 22

CMD ["/usr/local/bin/${APP_NAME}"]
