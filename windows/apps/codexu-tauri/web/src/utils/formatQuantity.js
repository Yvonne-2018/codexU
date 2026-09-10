const UNAVAILABLE_QUANTITY = '--';
const QUANTITY_UNITS = ['', 'K', 'M', 'B'];

function roundToOneDecimal(value) {
  return Number(value.toFixed(1));
}

function withSign(value, sign) {
  return sign < 0 ? `-${value}` : value;
}

export function formatQuantity(value) {
  if (value == null || !Number.isFinite(value)) {
    return UNAVAILABLE_QUANTITY;
  }

  const sign = Math.sign(value);
  let remaining = Math.abs(value);

  if (remaining < 1_000) {
    return withSign(String(Math.round(remaining)), sign);
  }

  let unitIndex = 0;
  while (unitIndex < QUANTITY_UNITS.length - 1 && remaining >= 1_000) {
    remaining /= 1_000;
    unitIndex += 1;
  }

  let compact = roundToOneDecimal(remaining);
  while (compact >= 1_000 && unitIndex < QUANTITY_UNITS.length - 1) {
    compact = roundToOneDecimal(compact / 1_000);
    unitIndex += 1;
  }

  return withSign(`${compact.toFixed(1)}${QUANTITY_UNITS[unitIndex]}`, sign);
}
