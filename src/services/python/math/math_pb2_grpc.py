# Generated by the gRPC Python protocol compiler plugin. DO NOT EDIT!
"""Client and server classes corresponding to protobuf-defined services."""
import grpc

import math_pb2 as math__pb2


class SimpleMathStub(object):
    """The simple math service definition.
    """

    def __init__(self, channel):
        """Constructor.

        Args:
            channel: A grpc.Channel.
        """
        self.Mul3 = channel.unary_unary(
                '/math.SimpleMath/Mul3',
                request_serializer=math__pb2.Number.SerializeToString,
                response_deserializer=math__pb2.Number.FromString,
                )
        self.Add2 = channel.unary_unary(
                '/math.SimpleMath/Add2',
                request_serializer=math__pb2.Number.SerializeToString,
                response_deserializer=math__pb2.Number.FromString,
                )


class SimpleMathServicer(object):
    """The simple math service definition.
    """

    def Mul3(self, request, context):
        """3*x
        """
        context.set_code(grpc.StatusCode.UNIMPLEMENTED)
        context.set_details('Method not implemented!')
        raise NotImplementedError('Method not implemented!')

    def Add2(self, request, context):
        """y+2
        """
        context.set_code(grpc.StatusCode.UNIMPLEMENTED)
        context.set_details('Method not implemented!')
        raise NotImplementedError('Method not implemented!')


def add_SimpleMathServicer_to_server(servicer, server):
    rpc_method_handlers = {
            'Mul3': grpc.unary_unary_rpc_method_handler(
                    servicer.Mul3,
                    request_deserializer=math__pb2.Number.FromString,
                    response_serializer=math__pb2.Number.SerializeToString,
            ),
            'Add2': grpc.unary_unary_rpc_method_handler(
                    servicer.Add2,
                    request_deserializer=math__pb2.Number.FromString,
                    response_serializer=math__pb2.Number.SerializeToString,
            ),
    }
    generic_handler = grpc.method_handlers_generic_handler(
            'math.SimpleMath', rpc_method_handlers)
    server.add_generic_rpc_handlers((generic_handler,))


 # This class is part of an EXPERIMENTAL API.
class SimpleMath(object):
    """The simple math service definition.
    """

    @staticmethod
    def Mul3(request,
            target,
            options=(),
            channel_credentials=None,
            call_credentials=None,
            insecure=False,
            compression=None,
            wait_for_ready=None,
            timeout=None,
            metadata=None):
        return grpc.experimental.unary_unary(request, target, '/math.SimpleMath/Mul3',
            math__pb2.Number.SerializeToString,
            math__pb2.Number.FromString,
            options, channel_credentials,
            insecure, call_credentials, compression, wait_for_ready, timeout, metadata)

    @staticmethod
    def Add2(request,
            target,
            options=(),
            channel_credentials=None,
            call_credentials=None,
            insecure=False,
            compression=None,
            wait_for_ready=None,
            timeout=None,
            metadata=None):
        return grpc.experimental.unary_unary(request, target, '/math.SimpleMath/Add2',
            math__pb2.Number.SerializeToString,
            math__pb2.Number.FromString,
            options, channel_credentials,
            insecure, call_credentials, compression, wait_for_ready, timeout, metadata)
