FROM alpine:latest

RUN mkdir /jotunheim
COPY jotunheim /jotunheim/

EXPOSE 7200
ENV RUST_LOG info
ENV JH_ADDR "0.0.0.0:7200"
ENV JH_NAME "office"

WORKDIR /jotunheim
CMD ["/jotunheim/jotunheim"]