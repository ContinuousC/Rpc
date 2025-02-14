################################################################################
# Copyright ContinuousC. Licensed under the "Elastic License 2.0".             #
################################################################################

import os, socket
from server import RpcTransport, RpcConnection

class UnixTransport(RpcTransport):

    def __init__(self, path):
        self.__path = path
        self.__sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.__sock.bind(path)
        self.__sock.listen(10)

    def fileno(self):
        return self.__sock.fileno()

    def accept(self):
        (sock, addr) = self.__sock.accept()
        return SocketConnection(sock, addr)

    def close(self):
        self.__sock.close()
        os.unlink(self.__path)

class SocketConnection(RpcConnection):

    def __init__(self, sock, addr):
        self.__sock = sock
        self.__addr = addr

    def fileno(self):
        return self.__sock.fileno()

    def recv(self, size = 1024):
        return self.__sock.recv(1024)

    def send(self, data):
        self.__sock.sendall(data)

    def shutdown(self):
        self.__sock.shutdown(socket.SHUT_RD)

    def close(self):
        self.__sock.close()

# class PipeConnection(Connection):

#     def __init__(self, inp, outp):
#         self.__inp = inp
#         self.__outp = outp

#     def recv(self, size = 1024):
#         return self.__inp.read()

#     def send(self, data):
#         self.__outp.write(data)
