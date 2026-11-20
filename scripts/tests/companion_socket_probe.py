#!/usr/bin/env python3
"""Real helper socket contract. Synthetic isolated state, never the live room."""
import argparse, base64, hashlib, http.client, json, os, pathlib, secrets, socket, ssl, subprocess, tempfile, time

def port():
    with socket.socket() as s: s.bind(('127.0.0.1',0)); return s.getsockname()[1]

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('binary'); args=parser.parse_args()
    ports=[port() for _ in range(4)]; http_port,ws,udp,tls=ports
    owner=secrets.token_urlsafe(32); device=secrets.token_urlsafe(32); proof=secrets.token_urlsafe(32)
    context=ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT); context.check_hostname=False; context.verify_mode=ssl.CERT_NONE
    count=0
    def check(condition,label):
        nonlocal count
        assert condition,label
        count+=1; print('PASS',label,flush=True)
    with tempfile.TemporaryDirectory(prefix='ruview-companion-probe-') as directory:
        root=pathlib.Path(directory); proc=None; log=(root/'server.log').open('w')
        def start():
            env=os.environ.copy();env['RUVIEW_API_TOKEN']=owner
            p=subprocess.Popen([str(pathlib.Path(args.binary).resolve()),'--source','simulated','--bind-addr','127.0.0.1','--http-port',str(http_port),'--ws-port',str(ws),'--udp-port',str(udp),'--companion-port',str(tls),'--installation-id','companion-probe','--data-dir',directory,'--no-mdns','--no-edge-registry'],env=env,stdout=log,stderr=subprocess.STDOUT)
            for _ in range(200):
                if p.poll() is not None: raise RuntimeError('helper exited; '+(root/'server.log').read_text()[-1600:])
                try:
                    with socket.create_connection(('127.0.0.1',http_port),timeout=.2):return p
                except OSError:time.sleep(.05)
            raise RuntimeError('helper did not bind')
        def request(path,method='GET',body=None,token=None,secure=False,pin=None):
            conn=http.client.HTTPSConnection('127.0.0.1',tls,context=context,timeout=5) if secure else http.client.HTTPConnection('127.0.0.1',http_port,timeout=5)
            conn.connect()
            if secure and pin: assert hashlib.sha256(conn.sock.getpeercert(binary_form=True)).hexdigest()==pin,'TLS pin changed'
            headers={'Content-Type':'application/json'}
            if token:headers['Authorization']='Bearer '+token
            if secure:headers['X-RuView-Installation-ID']='companion-probe'
            conn.request(method,path,json.dumps(body) if body is not None else None,headers)
            res=conn.getresponse(); status=res.status;raw=res.read();conn.close()
            try:value=json.loads(raw)
            except ValueError:value={}
            return status,value
        try:
            proc=start()
            check(request('/api/v1/companion')[0]==403,'owner endpoint rejects absent credential')
            status,invite=request('/api/v1/companion/invites','POST',{},owner)
            check(status==200 and invite['schema']=='ruview.companion.invite.v1','owner creates bounded invitation')
            pin=invite['cert_sha256']
            payload={k:invite[k] for k in ['installation_id','invite_id','invite_secret']};payload.update(device_id=device,device_name='Contract probe',pairing_secret=proof)
            status,enrolled=request('/companion/v1/enroll','POST',payload,secure=True,pin=pin)
            check(status==200 and enrolled['status']=='pending','pinned enrollment pending owner')
            session={'installation_id':'companion-probe','device_id':device,'pairing_secret':proof}
            check(request('/companion/v1/session','POST',session,secure=True,pin=pin)[1]['status']=='pending','QR cannot autoapprove')
            check(request('/companion/v1/enroll','POST',payload,secure=True,pin=pin)[0]>=400,'invitation cannot replay')
            check(request('/api/v1/companion/requests/'+enrolled['request_id']+'/approve','POST',{'comparison_code':enrolled['comparison_code']},owner)[0]==200,'owner approves matched request')
            status,access=request('/companion/v1/session','POST',session,secure=True,pin=pin);token=access['access_token']
            check(status==200,'device obtains read session')
            check(request('/api/v1/status',token=token,secure=True,pin=pin)[0]==200,'read token works over pinned TLS')
            check(request('/api/v1/status',token=token)[0]==401,'companion token rejected over plaintext')
            check(request('/api/v1/companion',token=owner,secure=True,pin=pin)[0] in (403,404),'owner token cannot manage over TLS LAN listener')
            check(request('/api/v1/ws-ticket','POST',{},token,True,pin)[0]==403,'legacy browser ticket denied')
            check(request('/api/v1/calibration/start','POST',{},token,True,pin)[0]==403,'read grant cannot calibrate')
            check(request('/api/v1/companion/devices/'+device+'/calibration','POST',{},owner)[0]==200,'owner grants calibration separately')
            status,result=request('/api/v1/calibration/start','POST',{'binding_digest':'a'*64,'source_node_ids':[1]},token,True,pin)
            check(status==200 and result.get('error_code')=='shared_room_binding_required','calibration rejects unshared room')
            check(request('/api/v1/calibration/reset','POST',{},token,True,pin)[0]==403,'calibration grant is not administrator')
            room={'schema':'ruview.companion.room.v1','id':'room-1','name':'Probe room','widthM':5,'lengthM':6,'heightM':2.6,'nodes':[{'nodeId':1,'slot':'north_west','xM':.45,'yM':.45,'role':'receiver','headingDeg':90,'orientationConfirmed':True,'confirmedAtUnixMs':1}],'accessPoint':{'slot':'north_east','xM':4.55,'yM':.45,'provenance':'operator_entered','confirmedAtUnixMs':1}}
            status,published=request('/api/v1/companion/room','PUT',{'expected_revision':0,'room':room},owner)
            check(status==200 and published['revision']==1,'owner publishes canonical room')
            check(request('/companion/v1/room',token=token,secure=True,pin=pin)[1]==published,'paired phone reads exact shared room')
            check(request('/api/v1/companion/room','PUT',{'expected_revision':0,'room':room},owner)[0]==409,'room publication rejects stale revision')
            raw=socket.create_connection(('127.0.0.1',tls),timeout=5);stream=context.wrap_socket(raw,server_hostname='localhost')
            check(hashlib.sha256(stream.getpeercert(binary_form=True)).hexdigest()==pin,'stream server pin matches')
            key=base64.b64encode(secrets.token_bytes(16)).decode()
            stream.sendall(('GET /ws/sensing HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: '+key+'\r\nAuthorization: Bearer '+token+'\r\nX-RuView-Installation-ID: companion-probe\r\n\r\n').encode())
            check(b'101 Switching Protocols' in stream.recv(4096),'authenticated live WebSocket upgrades')
            proc.terminate();proc.wait(timeout=10);proc=start()
            check(request('/api/v1/status',token=token,secure=True,pin=pin)[0]==401,'restart invalidates ephemeral access token')
            status,access=request('/companion/v1/session','POST',session,secure=True,pin=pin);token=access['access_token']
            check(status==200 and request('/api/v1/status',token=token,secure=True,pin=pin)[0]==200,'durable device proof refreshes after restart')
            stream.close()
            raw=socket.create_connection(('127.0.0.1',tls),timeout=5);stream=context.wrap_socket(raw,server_hostname='localhost');stream.sendall(('GET /ws/sensing HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: '+key+'\r\nAuthorization: Bearer '+token+'\r\nX-RuView-Installation-ID: companion-probe\r\n\r\n').encode());stream.recv(4096)
            check(request('/api/v1/companion/devices/'+device,'DELETE',token=owner)[0]==200,'owner revokes phone')
            check(request('/api/v1/status',token=token,secure=True,pin=pin)[0]==401,'revocation blocks existing read token')
            check(request('/companion/v1/session','POST',session,secure=True,pin=pin)[0]==401,'revocation blocks durable refresh')
            stream.settimeout(3);closed=False
            try:
                for _ in range(100):
                    if not stream.recv(65536):closed=True;break
            except (ssl.SSLEOFError,ConnectionResetError):closed=True
            check(closed,'revocation closes already-open WebSocket');stream.close()
            print('COMPANION SOCKET CONTRACT PASS',count)
        finally:
            if proc and proc.poll() is None:proc.terminate();proc.wait(timeout=10)
            log.close()
if __name__=='__main__':main()
