export const idlFactory = ({ IDL }) => {
  const Result = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : IDL.Text });
  return IDL.Service({
    'watch_usdc_transfer_get' : IDL.Func([], [Result], []),
    'watch_usdc_transfer_is_polling' : IDL.Func([], [Result], []),
    'watch_usdc_transfer_poll_count' : IDL.Func([], [Result], []),
    'watch_usdc_transfer_start' : IDL.Func([], [Result], []),
    'watch_usdc_transfer_stop' : IDL.Func([], [Result], []),
  });
};
export const init = ({ IDL }) => { return []; };
