import { createSlice, PayloadAction } from "@reduxjs/toolkit";

export interface RatesState {
  btcPrice: number | null;
  bdxPrice: number | null;
}

const initialState: RatesState = {
  btcPrice: null,
  bdxPrice: null,
};

const ratesSlice = createSlice({
  name: "rates",
  initialState,
  reducers: {
    setBtcPrice: (state, action: PayloadAction<number>) => {
      state.btcPrice = action.payload;
    },
    setBeldexPrice: (state, action: PayloadAction<number>) => {
      state.bdxPrice = action.payload;
    },
  },
});

export const { setBtcPrice, setBeldexPrice } = ratesSlice.actions;

export default ratesSlice.reducer;
