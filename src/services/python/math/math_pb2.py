# -*- coding: utf-8 -*-
# Generated by the protocol buffer compiler.  DO NOT EDIT!
# source: math.proto
"""Generated protocol buffer code."""
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from google.protobuf import reflection as _reflection
from google.protobuf import symbol_database as _symbol_database
# @@protoc_insertion_point(imports)

_sym_db = _symbol_database.Default()




DESCRIPTOR = _descriptor.FileDescriptor(
  name='math.proto',
  package='math',
  syntax='proto3',
  serialized_options=None,
  create_key=_descriptor._internal_create_key,
  serialized_pb=b'\n\nmath.proto\x12\x04math\"\x17\n\x06Number\x12\r\n\x05value\x18\x01 \x01(\x05\x32T\n\nSimpleMath\x12\"\n\x04Mul3\x12\x0c.math.Number\x1a\x0c.math.Number\x12\"\n\x04\x41\x64\x64\x32\x12\x0c.math.Number\x1a\x0c.math.Numberb\x06proto3'
)




_NUMBER = _descriptor.Descriptor(
  name='Number',
  full_name='math.Number',
  filename=None,
  file=DESCRIPTOR,
  containing_type=None,
  create_key=_descriptor._internal_create_key,
  fields=[
    _descriptor.FieldDescriptor(
      name='value', full_name='math.Number.value', index=0,
      number=1, type=5, cpp_type=1, label=1,
      has_default_value=False, default_value=0,
      message_type=None, enum_type=None, containing_type=None,
      is_extension=False, extension_scope=None,
      serialized_options=None, file=DESCRIPTOR,  create_key=_descriptor._internal_create_key),
  ],
  extensions=[
  ],
  nested_types=[],
  enum_types=[
  ],
  serialized_options=None,
  is_extendable=False,
  syntax='proto3',
  extension_ranges=[],
  oneofs=[
  ],
  serialized_start=20,
  serialized_end=43,
)

DESCRIPTOR.message_types_by_name['Number'] = _NUMBER
_sym_db.RegisterFileDescriptor(DESCRIPTOR)

Number = _reflection.GeneratedProtocolMessageType('Number', (_message.Message,), {
  'DESCRIPTOR' : _NUMBER,
  '__module__' : 'math_pb2'
  # @@protoc_insertion_point(class_scope:math.Number)
  })
_sym_db.RegisterMessage(Number)



_SIMPLEMATH = _descriptor.ServiceDescriptor(
  name='SimpleMath',
  full_name='math.SimpleMath',
  file=DESCRIPTOR,
  index=0,
  serialized_options=None,
  create_key=_descriptor._internal_create_key,
  serialized_start=45,
  serialized_end=129,
  methods=[
  _descriptor.MethodDescriptor(
    name='Mul3',
    full_name='math.SimpleMath.Mul3',
    index=0,
    containing_service=None,
    input_type=_NUMBER,
    output_type=_NUMBER,
    serialized_options=None,
    create_key=_descriptor._internal_create_key,
  ),
  _descriptor.MethodDescriptor(
    name='Add2',
    full_name='math.SimpleMath.Add2',
    index=1,
    containing_service=None,
    input_type=_NUMBER,
    output_type=_NUMBER,
    serialized_options=None,
    create_key=_descriptor._internal_create_key,
  ),
])
_sym_db.RegisterServiceDescriptor(_SIMPLEMATH)

DESCRIPTOR.services_by_name['SimpleMath'] = _SIMPLEMATH

# @@protoc_insertion_point(module_scope)
