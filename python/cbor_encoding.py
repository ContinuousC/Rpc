################################################################################
# Copyright ContinuousC. Licensed under the "Elastic License 2.0".             #
################################################################################

import cbor2

class CborEncoding(RpcEncoding):
    def decode(self, data):
        lines = data.split('\n')
        msgs = []
        for line in lines[:-1]:
            try: msgs.append(json.loads(line))
            except Exception as err:
                print >>sys.stderr, "Warning: ignoring invalid message: %s (%s: %s)" \
                    % (line, type(err).__name__, str(err))
        return msgs, lines[-1]

    def encode(self, msg):
        return cbor2.dumps(msg)
