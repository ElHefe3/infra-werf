"""Disposable Windows named-pipe SSH agent; signs with one synthetic key only.

No OS agent service or owner key store is read or modified. The private key is
owned by the loopback fixture and never traverses this protocol.
"""
import asyncio
import json
import os
import struct
import uuid
from asyncssh.packet import SSHPacket, Byte, UInt32, String

async def start_agent(directory, key):
    if os.name != 'nt': return None, lambda: None
    address = r'\\.\pipe\atlas-fixture-' + uuid.uuid4().hex
    peers = set()
    stats = {'identities': 0, 'signatures': 0, 'active': 0}
    def save(): (directory / 'agent-stats.json').write_text(json.dumps(stats), encoding='utf-8')
    class Agent(asyncio.Protocol):
        def connection_made(self, transport):
            self.transport=transport; self.buffer=bytearray(); peers.add(self)
            stats['active']+=1; save()
        def connection_lost(self, error):
            peers.discard(self); stats['active']-=1; save()
        def data_received(self, data):
            self.buffer.extend(data)
            if len(self.buffer)>131072: self.transport.close(); return
            while len(self.buffer)>=4:
                size=struct.unpack('>I',self.buffer[:4])[0]
                if not 0<size<=65536: self.transport.close(); return
                if len(self.buffer)<size+4: return
                payload=bytes(self.buffer[4:size+4]); del self.buffer[:size+4]
                response=Byte(5)
                try:
                    packet=SSHPacket(payload); kind=packet.get_byte()
                    if kind==11:
                        packet.check_end(); stats['identities']+=1
                        response=Byte(12)+UInt32(1)+String(key.public_data)+String(b'Atlas synthetic fixture')
                    elif kind==13:
                        public=packet.get_string(); message=packet.get_string(); flags=packet.get_uint32(); packet.check_end()
                        if public==key.public_data and flags==0 and len(message)<=32768:
                            response=Byte(14)+String(key.sign(message,b'ssh-ed25519')); stats['signatures']+=1
                except (ValueError,IndexError): pass
                save(); self.transport.write(UInt32(len(response))+response)
    listeners=await asyncio.get_running_loop().start_serving_pipe(Agent,address)
    save()
    def close():
        for listener in listeners: listener.close()
        for peer in list(peers): peer.transport.close()
    return address.replace('\\','/'),close
