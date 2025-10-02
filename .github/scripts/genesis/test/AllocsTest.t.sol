// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

import {TestERC20} from "zenith/test/Helpers.t.sol";
import {Test, console2} from "forge-std/Test.sol";

contract AllocsTest is Test {
    // the path to the genesis file being tested
    string constant ALLOCS_FILE_PATH = "../../../src/config/genesis/pecorino.genesis.json";
    // the chain ID in the genesis file being tested
    uint256 constant CHAIN_ID = 14174;
    // the Minter key that the Node uses to mint ERC20 tokens on EnterToken events
    address constant MINTER = 0x00000000000000000000746f6b656E61646d696E;

    // the RollupPassage contract
    address constant ROLLUP_PASSAGE = 0x0000000000007369676E65742D70617373616765;
    // the RollupOrders contract
    address constant ROLLUP_ORDERS = 0x000000000000007369676E65742D6f7264657273;
    // the Permit2 contract
    address constant PERMIT2 = 0x000000000022D473030F116dDEE9F6B43aC78BA3;
    // the WETH9 Wrapped native token contract, pre-deployed for convenience
    address constant WRAPPED = 0x0000000000000000007369676e65742D77757364;
    // the WBTC token contract, supported by EnterToken
    address constant WBTC = 0x0000000000000000007369676e65742D77627463;
    // the WETH token contract, supported by EnterToken
    address constant WETH = 0x0000000000000000007369676e65742d77657468;

    address[] tokens = [WBTC, WETH];

    function _mintTokens(address _recipient, uint256 _amount) internal {
        vm.startPrank(MINTER);
        for (uint256 i = 0; i < tokens.length; i++) {
            TestERC20(tokens[i]).mint(_recipient, _amount);
        }
        vm.stopPrank();
    }

    function _approveTokens(address _owner, uint256 _amount, address _spender) internal {
        vm.startPrank(_owner);
        for (uint256 i = 0; i < tokens.length; i++) {
            TestERC20(tokens[i]).approve(_spender, _amount);
        }
        vm.stopPrank();
    }
}
