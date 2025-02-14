################################################################################
# Copyright ContinuousC. Licensed under the "Elastic License 2.0".             #
################################################################################

import sys
import uuid
import select
import socket
import threading
import traceback

from multiprocessing import Queue
from Queue import Empty

class RpcServer(object):

    def __init__(self, service, transport, encoding, nworkers = 10, debug = False):
        self.__service = service
        self.__transport = transport
        self.__encoding = encoding
        self.__queue = Queue()
        self.__sessions = {}
        self.__handlers = []
        self.__debug = debug

        self.__run = True
        self.__interrupt, self.__interrupt_write = socket.socketpair()

        self.__workers = [threading.Thread(target = self.__worker, args = (i,))
                          for i in range(nworkers)]
        for worker in self.__workers: worker.start()

    def start(self):
        if isinstance(self.__transport, RpcTransport):
            self.__acceptor = threading.Thread(target = self.__accept)
            self.__acceptor.start()
        elif isinstance(self.__transport, RpcConnection):
            self.__acceptor = threading.Thread(target = self.__handle, args = (self.__transport,))
            self.__acceptor.start()
        else:
            raise RpcException("transport should either be an instance of "
                               "RpcTransport or RpcConnection")

    def send_shutdown(self):
        if self.__run:
            self.__run = False
            self.__interrupt_write.send(b'\0')

    def await_shutdown(self):
        self.__acceptor.join()
        for worker in self.__workers: worker.join()
        for handler in self.__handlers: handler.join()

    def shutdown(self):
        self.send_shutdown()
        self.await_shutdown()

    def __accept(self):
        while self.__run:
            r,w,x = select.select([self.__interrupt, self.__transport], [], [])
            if self.__transport not in r: break
            conn = self.__transport.accept()
            thread = threading.Thread(target = self.__handle, args = (conn,))
            thread.start()
            self.__handlers.push(thread)
            self.__handlers = [handler for handler in self.__handlers if handler.is_alive()]
        self.__transport.close()

    def __handle(self, conn):
        session_id = uuid.uuid4()
        self.__sessions[session_id] = (conn, self.__service.session(conn))
        data = ""
        while self.__run:
            r,w,x = select.select([self.__interrupt, conn], [], [])
            if conn not in r: break
            block = conn.recv(1024)
            if len(block) == 0: break
            msgs,data = self.__encoding.decode(data + block)
            for msg in msgs:
                self.__queue.put((session_id, msg))
        print >>sys.stderr, "Connection closed"
        del self.__sessions[session_id]
        conn.shutdown()

    def __worker(self, n):
        while self.__run or not self.__queue.empty():
            if self.__run:
                r,w,x = select.select([self.__interrupt, self.__queue._reader], [], [])
                if self.__queue._reader not in r: continue
            try:
                (session_id, msg) = self.__queue.get(False)
                (conn, sess) = self.__sessions[session_id]
            except KeyError: continue
            except Empty: continue
            try:
                res = { "Ok": self.__service.request(sess, msg['request']) }
            except Exception as err:
                res = { "Err": (traceback.format_exc(err) if self.__debug \
                                else "%s: %s" % (type(err).__name__, str(err))) }
            data = self.__encoding.encode({'req_id': msg['req_id'], 'response': res})
            r,w,x = select.select([self.__interrupt], [conn], [])
            if conn not in w: break
            conn.send(data)

### Interfaces and base classes

class RpcException(Exception):
    pass

class RpcService(object):
    def request(self, req):
        raise RpcException('unimplemented')

class RpcEncoding(object):
    def encode(self, msg):
        raise RpcException("unimplemented")
    def decode(self, data):
        raise RpcException("unimplemented")

class RpcTransport(object):
    def accept(self):
        raise RpcException("unimplemented")

class RpcConnection(object):
    def recv(self, size = 1024):
        raise RpcException("unimplemented")
    def send(self, data):
        raise RpcException("unimplemented")
    def shutdown(self):
        raise RpcException("unimplemented")
