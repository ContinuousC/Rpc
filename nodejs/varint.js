/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

/* Variable-length unsigned integer handling.
 * See https://sqlite.org/src4/doc/trunk/www/varint.wiki
 * 
 * Note that since javascript numbers are 64-bit floats, this only
 * works with numbers smaller than 2**52 . Additionally, we cannot use
 * bitwise operations, because they are defined as operations on
 * 32-bit signed integers.
 *
 */

/* Encode an unsigned integer number to a buffer. */
function varuint_encode(n) {
    if (n < 0 || n >= 2**52) {
	throw new VarIntError("tried to encode a number outside the supported range.");
    } else if (n <= 240) {
	return Buffer.from([n]);
    } else if (n <= 2287) {
	const n0 = n - 240;
	return Buffer.from([ n0 / 256 + 241,
			     n0 % 256
			   ]);
    } else if (n <= 67823) {
	let n0 = n - 2288;
	return Buffer.from([ 249,
			     n0 / 256,
			     n0 % 256
			   ]);
    } else {
	let b;
	for (b = []; n > 0; n = Math.floor(n / 256)) {
	    b.push(n % 256);
	}
	return Buffer.from([b.length + 247].concat(b.reverse()))
    }
}

/* Find the length of a varint-encoded number based
 * on the first byte.
 */
function varuint_len(b0) {
    return b0 <= 240 ? 1
	: b0 <= 248 ? 2
	: b0 - 246;
}

/* Decode an unsigned integer number from a buffer. */
function varuint_decode(b) {
    if (b[0] > 254 || (b[0] == 254 && b[1] > 15)) {
	throw new VarIntError("tried to decode a number outside the supported range.");
    } else if (b[0] <= 240) {
	return b[0];
    } else if (b[0] <= 248) {
	return 240 + 256 * (b[0] - 241) + b[1];
    } else if (b[0] == 249) {
	return 2288 + 256 * b[1] + b[2];
    } else {
	return b.slice(1, b[0] - 246)
	    .reduce((n,i) => n * 256 + i, 0);
    }
}

class VarIntError extends Error {}

module.exports = { varuint_encode, varuint_decode, varuint_len, VarIntError };
