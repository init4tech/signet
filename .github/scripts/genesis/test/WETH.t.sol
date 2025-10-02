// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

// utils
import {AllocsTest} from "./AllocsTest.t.sol";
import {IWETH} from "./interfaces/IWETH.sol";

contract AllocsWrappedTokenTest is AllocsTest {
    IWETH target;

    fallback() external payable {}
    receive() external payable {}

    function setUp() public {
        vm.loadAllocs(ALLOCS_FILE_PATH);
        target = IWETH(payable(WRAPPED));
    }

    function test_metadata() external {
        assertEq(target.name(), "Wrapped USD", "name");
        assertEq(target.symbol(), "WUSD", "symbol");
        assertEq(target.decimals(), 18, "decimals");
    }

    function test_deposit() external {
        assertEq(target.balanceOf(address(this)), 0);
        assertEq(target.totalSupply(), 0);

        // transfer eth to target using call
        payable(address(target)).call{value: 100}("");

        // should mint ERC20 token
        assertEq(target.balanceOf(address(this)), 100);
        assertEq(target.totalSupply(), 100);

        // deposit directly
        target.deposit{value: 100}();

        // should mint ERC20 token
        assertEq(target.balanceOf(address(this)), 200);
        assertEq(target.totalSupply(), 200);
    }

    function test_withdraw() external {
        // deposit
        target.deposit{value: 100}();

        // should mint ERC20 token
        assertEq(target.balanceOf(address(this)), 100);
        assertEq(target.totalSupply(), 100);

        // get eth balance before withdraw
        uint256 ethBalance = address(this).balance;

        // withdraw
        target.withdraw(100);

        // ERC20 token should be burned
        assertEq(target.balanceOf(address(this)), 0);
        assertEq(target.totalSupply(), 0);
        // eth should be transferred
        assertEq(address(this).balance, ethBalance + 100);
    }
}
