/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

const cbor = require('cbor');
const tls = require('tls'); 
const net = require('net');
const fs = require('fs').promises;
const {EventEmitter} = require("events");

const {varuint_decode, varuint_encode, varuint_len} = require("./varint")

class CborRpcClient extends EventEmitter {
    constructor(opts) {
	super();

	if (!('port' in opts)) {
	    throw new RpcError("missing port option");
	}

	this.req_id = 0;
	this.address = opts.address || 'localhost';
	this.port = opts.port
	this.ca_path = opts.ca_path || 'certs/ca.crt';
	this.cert_path = opts.cert_path || 'certs/client.crt';
	this.key_path = opts.key_path || 'certs/client.key';
	this.retry_interval = opts.retry_interval || 10000;
	this.request_timeout = opts.request_timeout || 10000;
	this.verbose = opts.verbose || false;

	this.stream = null;
	this.retry_id = null;
	this.connected = false;
	this.shutdown = true; // When first created, we are not retrying
	
	if ([undefined, null, true].includes(opts.handle_messages)) {
	    this.on('message', msg => {
		try {
		    const {req_id, response} = msg;
		    this.emit('response-' + req_id, response);
		} catch (e) {
		    console.log("Warning: invalid message: " + e);
		}
	    })
	};

	this.on('disconnected', () => {
	    if (!this.shutdown) {
		    this.retry_id = setTimeout(async () => {
          try {
            await this.connect()
          }catch(error){
            console.log(`Could not connect to ${this.address}:${this.port}, retrying in ${this.retry_interval} `)
          }
        }, this.retry_interval);
	    }
	})
    }

    async connect() {

	this.shutdown = false;

	if (this.retry_id) {
	    clearTimeout(this.retry_id);
	    this.retry_id = null;
	}
	
	if (this.verbose) {
	    console.log(`Connecting to ${this.address}:${this.port}`);
	}

        this.stream = await tls.connect({
            host: this.address, port: this.port,
            ca: await fs.readFile(this.ca_path),
            key: await fs.readFile(this.key_path),
            cert: await fs.readFile(this.cert_path),
        });

	if (this.verbose) {
	    console.log("Network stream connected!");
	}

	/* Install hangup / error handlers. */
	
        this.stream.on('end', () => {
            if (this.verbose) {
                console.log(`Disconnected ${this.address}:${this.port}`);
            }
	    this.stream = null;
	    this.connected = false;
            this.emit('disconnected', "The remote hung up unexpectedly");
        });

        this.stream.on('error', (err) => {
            if (this.verbose) {
                console.log('Socket error: ' + err);
            }
            this.stream = null;
	    this.connected = false;
            this.emit('disconnected', err);
        });

        /* Decode messages. */

        let buf = new Buffer.from([]);
        this.stream.on('data', data => {
	    buf = Buffer.concat([buf, Buffer.from(data)]);
            while(true) {
                if (buf.length < 1) return;
		let llen = varuint_len(buf[0]);
		if (buf.length < llen) return;
                let len = varuint_decode(buf);

                if (buf.length < llen + len) return;
                let msg = cbor.decodeFirstSync(buf.slice(llen, llen + len));
                buf = buf.slice(llen + len);

                this.emit('message', msg);
            }
        });

	/* Await connection. */
	
	await new Promise((resolve,reject) => {
            this.stream.once('secureConnect', () => {
		resolve();
	    });
            this.stream.once('error', () => {
		reject();
	    });
        });

	if (this.verbose) {
	    console.log("TLS negotiation succeeded!");
	}

	this.connected = true;
	this.emit("connected");
	
    }

    async disconnect() {
	this.shutdown = true;
	if (this.connected) {
	    const end = new Promise(resolve => this.stream.once('end', resolve));
	    this.stream.end();
	    await end;
	}
    }

    async request(request, options, ctx) {

        if (!this.connected) {
            throw new RpcError("Attempt to write in disconnected state!");
        }

        let req_id = this.req_id++;
        let ac = new AbortController();
        let timeout = this.timeout(ctx.timeout || this.request_timeout, ac);
        let response = this.response(req_id, ac);

        await this.write_msg({
            req_id,
            request: { [request]: options }
        });
        let res = await Promise.race([response, timeout]);

        return res;
    }

    response(req_id, ac) {
        return new Promise((resolve,reject) => {
            let listener = res => {
                if (ac && ac.aborted) {
                    //console.log('Debug: ignoring late response for ' + req_id);
                } else {
                    if ('Ok' in res) {
                        resolve(res.Ok);
                    } else if ('Err' in res) {
                        reject(new RpcError(res.Err))
                    } else {
                        console.log(JSON.stringify(res))
                        reject(`rpc call failed without an error message`)
                    }
                    if (ac) ac.abort();
                }
            };
            this.once('response-' + req_id, listener);
            if (ac) ac.signal.once('abort', ev => {
                super.off('response-' + req_id, listener);
                reject(ev);
            });
        });
    }

    timeout(ms, ac) {
        return new Promise((resolve,reject) => {
            let timeout = setTimeout(() => {
                reject("timeout");
                if (ac) ac.abort();
            }, ms);
            if (ac) ac.signal.once('abort', ev => {
                clearTimeout(timeout);
                //reject("aborted");
            });
        });
    }

    async write_msg(msg) {

        if (!this.stream) {
            throw "Attempt to write in disconnected state!";
        }

        //console.log('Debug: sending message ' + JSON.stringify(msg));
        let buf = await cbor.encodeAsync(msg);

        if (buf.length >= 2**32) {
            if (this.verbose) {
                console.log("Refusing to write message >= 4GB");
            }
            return;
        }

        await new Promise((resolve, reject) => this.stream.write(
	    Buffer.concat([varuint_encode(buf.length), buf]),
	    e => e === undefined ? resolve() : reject(e)));

    }

}

class RpcError extends Error {
    constructor(msg) {
	super(msg)
    }
}

/* No longer required after update to node >= 15 */

class AbortController {
    constructor() {
        this.aborted = false;
        this.signal = new EventEmitter();
    }
    abort() {
        if (!this.aborted) {
            this.aborted = true;
            this.signal.emit('abort');
        }
    }
}

module.exports = {
    CborRpcClient,
    RpcError,
};
