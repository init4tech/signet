// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

// test contract
import {IOrders} from "zenith/src/orders/IOrders.sol";
import {RollupOrders} from "zenith/src/orders/RollupOrders.sol";
// utils
import {AllocsTest} from "./AllocsTest.t.sol";
import {OrdersTest} from "zenith/test/Orders.t.sol";

contract AllocsOrderTest is AllocsTest, OrdersTest {
    function setUp() public override {
        vm.loadAllocs(ALLOCS_FILE_PATH);

        target = RollupOrders(ROLLUP_ORDERS);

        _mintTokens(address(this), amount * 1000);
        _approveTokens(address(this), amount * 1000, address(target));

        token = tokens[0];
        token2 = tokens[1];

        // setup Order Inputs/Outputs
        IOrders.Input memory input = IOrders.Input(token, amount);
        inputs.push(input);

        IOrders.Output memory output = IOrders.Output(token, amount, recipient, chainId);
        outputs.push(output);
    }

    // run tests written for local chain env, pointed to contracts deployed in genesis file
}
