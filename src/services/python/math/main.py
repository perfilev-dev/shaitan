
import grpc
from grpc_reflection.v1alpha import reflection

import math_pb2
import math_pb2_grpc

from concurrent import futures
import logging


class SimpleMath(math_pb2_grpc.SimpleMathServicer):

    def Mul3(self, request, context):
        return math_pb2.Number(value=3*request.value)

    def Add2(self, request, context):
        return math_pb2.Number(value=request.value+2)


def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    math_pb2_grpc.add_SimpleMathServicer_to_server(SimpleMath(), server)
    # the reflection service will be aware of "SimpleMath" and "ServerReflection" services.
    SERVICE_NAMES = (
        math_pb2.DESCRIPTOR.services_by_name['SimpleMath'].full_name,
        reflection.SERVICE_NAME,
    )
    reflection.enable_server_reflection(SERVICE_NAMES, server)
    server.add_insecure_port('[::]:50051')
    server.start()
    server.wait_for_termination()


if __name__ == '__main__':
    logging.basicConfig()
    serve()
