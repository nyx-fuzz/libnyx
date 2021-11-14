cargo build && gcc test.c target/debug/liblibnyx.a -o app -pthread -ldl -lrt && ./app
