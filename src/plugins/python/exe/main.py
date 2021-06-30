
import grpc
from grpc_reflection.v1alpha import reflection

import exe_pb2
import exe_pb2_grpc

from concurrent import futures
import logging


class Greeter(exe_pb2_grpc.GreeterServicer):

    def SayHello(self, request, context):
        return exe_pb2.HelloReply(message='Hello, %s!' % request.name)


def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    exe_pb2_grpc.add_GreeterServicer_to_server(Greeter(), server)
    # the reflection service will be aware of "Greeter" and "ServerReflection" services.
    SERVICE_NAMES = (
        exe_pb2.DESCRIPTOR.services_by_name['Greeter'].full_name,
        reflection.SERVICE_NAME,
    )
    reflection.enable_server_reflection(SERVICE_NAMES, server)
    server.add_insecure_port('[::]:50051')
    server.start()
    server.wait_for_termination()


if __name__ == '__main__':
    logging.basicConfig()
    serve()
